# PROJECT Explain: Versus Stable League r4

## 결정 배경

r3 장기 실행은 중반에 공격·Tetris·역사 상대 성능이 개선됐지만 후반에는 bootstrap 상대 성능만 유지되고 역사 상대 성능이 되돌아갔다. 구현 감사 결과 150개 snapshot에서 32개를 매번 등간격 재선택하는 방식은 snapshot 하나가 추가될 때 활성 상대 16개를 교체했다. PFSP 승률도 서로 다른 learner 세대의 평생 누적 전적이어서 현재 정책의 망각을 즉시 반영하지 못했다.

하루 단위 실험을 여러 번 소비하지 않도록 r4는 확인된 상호작용 병목을 한 실행 경로에서 함께 수정한다. r3 checkpoint와 재개 형식은 변경하지 않고 `versus-selfplay-ppo-v5`와 `checkpoints/versus-selfplay-r4`를 별도 사용한다.

## 안정적 상대 풀

- 활성 풀 상태를 `latest.pt`에 직렬화하고 glob 재층화를 금지한다.
- r3 update 700과 1050을 고정 anchor로 시작한다.
- 복구 snapshot은 10 update마다 계속 저장하지만 상대 승격은 50 update마다 한 번만 수행한다.
- 32개 중 최근 슬롯은 12개다. 넘친 recent는 archive로 내려가고, 용량 초과 시 시간상 가장 중복된 비보호 archive 하나만 제거한다.
- 한 승격에서 활성 membership 제거는 최대 하나이며 anchor는 제거되지 않는다.
- 진행 중인 match는 풀에서 상대가 빠져도 terminal까지 기존 checkpoint와 learner side를 유지한다.

## 최근 전적과 혼합 샘플링

상대 결과 `r in {0, 0.5, 1}`는 완료 update와 함께 최대 256개 저장한다. 현재 update `u`에서 결과 가중치는 `2^(-(u-u_j)/100)`이고 Beta(1,1) prior를 사용한다.

```text
p_i = (1 + sum(weight_j * result_j)) / (2 + sum(weight_j))
```

역사 슬롯은 balanced 40%, hard 30%, uniform 30%를 먼저 선택한다. balanced weight는 `4*p*(1-p)`, hard weight는 `1-p`, uniform은 1이며 기존 exponent와 최소 weight를 적용한다. 이 구성은 50% 부근의 학습 가능한 상대, 현재 약점, 잊기 방지 복습을 동시에 유지한다.

## critic 보강

r3의 critic explained variance가 대체로 약 0.03에 머문 반면 PPO KL·clip은 안정적이었다. actor·solo trunk와 공유되지 않는 `value_core`를 별도 optimizer group으로 분리하고 learning rate를 기본의 2배로 사용한다. PPO 4 epoch 뒤 value-only 4 epoch를 추가한다. 후보 scorer를 다시 실행하지 않는 작은 122→32→32→1 회귀이므로 전체 update 비용 증가는 제한적이다.

`value_postfit_explained_variance`, `value_extra_loss`, `value_extra_gradient_norm`을 별도로 기록한다. 이 변경은 terminal 목적이나 potential shaping을 바꾸지 않는다.

## 자동 champion 선택

장기 실행 종료 후 승격 snapshot을 최근 100 update의 bootstrap/historical 실제 전적으로 6개 이하 shortlist한다. shortlist, r3 anchor, r4 시작 reference를 동일 seed·좌우 교대·8/12/15 frame cadence에서 평가한다. 우선순위는 최악 상대 평균 score, 전체 score, completion rate, outgoing attack, 낮은 danger 순서다.

선택기는 `selection-report.json`과 `selected-model.pt`를 생성한다. 최신 snapshot은 후보일 뿐 자동 우대하지 않는다. 이 평가는 강도 선택 절차이며 통계적으로 최적 전략을 증명하지 않는다.

## 호환성과 검증 경계

- r1-r4 config payload와 progress v3는 그대로 유지한다.
- r5 config는 stable pool을 요구하는 progress v4만 재개한다.
- r3 재개는 `scripts/run-versus-selfplay-r3.ps1`, r2 재개는 `scripts/run-versus-selfplay-r2.ps1`을 사용한다.
- r4 smoke는 실제 r3 update 1050에서 시작해 3 update, stable promotion, value-only update, 4번째 update exact resume와 champion selection artifact 생성을 통과해야 한다.
