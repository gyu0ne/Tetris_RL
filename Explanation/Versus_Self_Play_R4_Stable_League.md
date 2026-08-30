# 1대1 자기대전 r4: 안정적 상대 리그와 자동 선택

## 1. 무엇이 바뀌었나

r4는 하루짜리 학습을 작은 수정마다 반복하지 않도록 r3에서 확인된 두 병목을 한 실행에 반영한다.

1. 과거 상대 풀의 절반이 주기적으로 바뀌던 현상을 제거한다.
2. 서로 다른 learner 세대의 평생 누적 승률 대신 최근 정책을 반영하는 시간 감쇠 전적을 사용한다.
3. 어려운 상대만 쫓지 않고 균형 상대·현재 약점·전체 복습을 혼합한다.
4. 설명력이 낮았던 critic을 별도 학습률과 추가 value pass로 보강한다.
5. 실행 종료 후 최신 모델을 그대로 쓰지 않고 cadence-robust 대국으로 최종 checkpoint를 자동 선택한다.

보상은 변경하지 않는다. 승패 `+1/-1/0`과 반대칭·terminal-normalized potential difference가 그대로 최종 목적이다.

## 2. 상대 풀

기본 풀 한도는 32개다. r3의 안정 후보 update 700과 종합 후보 update 1050을 제거할 수 없는 anchor로 시작한다. r4 snapshot은 복구를 위해 10 update마다 저장하지만, 상대 풀에는 50 update마다 하나만 승격한다.

최근 12개는 recent로 유지한다. recent가 넘치면 가장 오래된 항목을 archive로 내리고, 전체 한도를 넘을 때 시간적으로 가장 중복된 archive 하나만 제거한다. 따라서 승격 한 번에 상대 하나만 빠지며 풀 절반이 갑자기 바뀌지 않는다. 풀 membership, 역할, 추가 update, 결과 이력은 `latest.pt`에 저장되어 `-Resume`이 동일한 리그를 복원한다.

## 3. 최근 승률

대국 결과가 100 update 지날 때마다 영향이 절반이 되도록 계산한다.

```text
weight = 2^(-age / 100)
p = (1 + sum(weight * result)) / (2 + sum(weight))
result: win=1, draw=0.5, loss=0
```

오래 상대하지 않은 checkpoint는 무조건 강하거나 약하다고 고정되지 않고 `p=0.5` 쪽으로 돌아온다. 다시 표본될 가능성이 생기므로 현재 모델이 예전 전략을 잊었는지 재확인할 수 있다.

역사 상대 선택은 다음 세 채널을 섞는다.

- 40% balanced: `4*p*(1-p)`, 현재 승률 50% 부근 우선
- 30% hard: `1-p`, 현재 약한 상대 우선
- 30% uniform: 모든 활성 상대 균등 복습

전체 경기 구성은 r3와 같은 current 35%, historical 50%, bootstrap 15%다.

## 4. critic 보강

r3 로그에서 정책 update는 안정적이었지만 critic explained variance는 약 0.03이었다. r4는 기존 actor 가중치를 그대로 초기화하면서 `value_core`만 기본 learning rate의 2배로 학습한다. 각 PPO update 뒤 value-only epoch를 4회 추가한다.

추가 pass는 이미 수집한 122개 state feature와 return target만 사용하므로 평균 약 68개 후보를 다시 생성하거나 채점하지 않는다. 다음 로그를 함께 본다.

- `value_postfit_explained_variance`: 추가 학습 뒤 전체 rollout 설명력
- `value_extra_loss`: value-only 회귀 손실
- `value_extra_gradient_norm`: value-only gradient 크기

설명력이 올라가지 않거나 held-out 승률이 악화되면 최종 선택에서 해당 checkpoint가 탈락한다.

## 5. 실행

r4는 r3 산출물의 update 700과 1050 checkpoint가 필요하다. 기본 초기 모델은 update 1050이다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24
```

출력은 `checkpoints/versus-selfplay-r4`에 저장된다. 정상 종료 후 자동으로 후보 평가를 수행하여 다음 파일을 만든다.

```text
selected-model.pt      최종 선택된 실사용 모델
selection-report.json  shortlist, 모든 대국과 선택 근거
latest.pt              정확 재개용 전체 상태
latest-model.pt        가장 최근 추론 모델
model.pt               학습 종료 시점 모델
snapshots/             10 update 간격 복구·후보 파일
```

중단 후 재개:

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile max -Hours 24 -Resume
```

학습만 짧게 확인하고 자동 평가를 생략할 때만 `-SkipSelection`을 사용한다. 실제 장기 실행에서는 기본 자동 선택을 유지한다.

```powershell
./scripts/run-versus-selfplay.ps1 -ResourceProfile balanced -Hours 1 -MaxUpdates 10 -SkipSelection
```

## 6. 자동 선택

선택기는 승격 snapshot 중 훈련 로그의 강건성이 높은 모델, 최신 모델, 시간상 중간 모델을 합쳐 최대 6개를 선별한다. 각 후보를 다음 상대와 대국시킨다.

- r3 update 700
- r3 update 1050
- r4 시작 reference
- 나머지 shortlist 후보

각 대국은 같은 seed를 좌우 진영에서 반복하고 8·12·15 frame cadence를 모두 사용한다. 최악 상대 score를 먼저 최대화하고, 전체 score, 완료율, outgoing attack, 낮은 danger 순서로 동률을 해소한다. 따라서 공격량만 높은 모델이나 마지막 snapshot이 자동 승격되지 않는다.

평가 규모를 바꾸려면 다음 인수를 사용한다.

```powershell
./scripts/run-versus-selfplay.ps1 `
  -ResourceProfile max `
  -Hours 24 `
  -SelectionSeeds 12 `
  -SelectionHorizon 2500
```

## 7. 이전 실행 보존

- r3를 그대로 재개: `./scripts/run-versus-selfplay-r3.ps1 ... -Resume`
- r2를 그대로 재개: `./scripts/run-versus-selfplay-r2.ps1 ... -Resume`

r3 `latest.pt`를 r4로 progress-resume하지 않는다. r4는 r3 추론 모델의 가중치만 초기화에 사용하고 optimizer, 경기, 최근 전적과 stable pool은 새로 시작한다.
