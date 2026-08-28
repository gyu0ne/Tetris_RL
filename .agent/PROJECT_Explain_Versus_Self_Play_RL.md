# PROJECT Explain: Versus Self-Play RL

## r2 확정 설계 (2026-08-28)

r1의 단일 최고 모델 가정을 폐기한다. 실제 held-out 대국에서 update 200, 230, 310이 순환 우위를 보였고 reference가 공격량이 더 낮으면서도 update 200을 이겼으므로, 공격량 자체는 목표가 아니라 승리 원인의 진단값이다.

- 목적은 계속 `win=+1, loss=-1, draw=0`인 zero-sum terminal outcome이다. attack/line/T-spin 보너스는 최적 정책을 바꿀 수 있으므로 reward에 직접 더하지 않는다.
- potential shaping은 `0.05 * (0.9995*Phi(s')-Phi(s))`로 축소한다. `Phi(terminal)=0`, player-swap 반대칭, `|Phi|<=1`을 유지하므로 stochastic-game equilibrium을 보존한다. 한 transition shaping 절댓값 상한은 `0.099975`다.
- r1의 `gamma=0.997`은 1,400수 뒤 승패를 `0.01490`으로 줄였다. r2의 `gamma=0.9995`는 같은 거리를 `0.49650`으로 보존하고 반감기는 `1,385.95`수다.
- GAE는 `lambda=0.995`로 높인다. 지수 trace 유효 길이 `1/(1-gamma*lambda)`는 r1의 `18.92`에서 `181.90`수로 늘어난다. 이는 장기 승패 전달을 개선하지만 분산도 늘리므로 32경기, 512-step rollout과 함께 사용한다.
- actor는 솔로 scorer에 별도 문맥 점수를 단순 합산하던 구조를 폐기한다. 솔로 10개 특징과 대전/보드/조각 66개 특징을 함께 보는 `76→64→32→1` residual을 사용하고 마지막 층을 0으로 초기화한다. 따라서 시작 정책은 솔로 정책과 정확히 같지만 이후에는 보드 모양과 가비지 상황의 상호작용을 표현할 수 있다.
- 관측에는 후보 착지 후 10열 높이, 10열 구멍, hold 사용 여부, 현재/hold/preview 3개 조각 one-hot을 추가한다. critic은 양쪽의 전역 압력 6개, 열 높이·구멍, 조각 문맥을 모두 보고 `V(swap(s))=-V(s)`를 구조적으로 강제한다.
- 솔로 trunk의 learning rate는 residual/value의 0.1배다. 고정 모방 teacher와의 `KL(teacher || learner)` 계수는 `0.02→0.001`로 500 update 동안 감소시킨다. 이것은 안전한 줄 지우기를 초기 탐색 기준으로 유지하되 승패 학습이 teacher를 추월할 여지를 남기는 kickstarting 보조 목적이다.
- 상대 구성은 current self-play 35%, historical PFSP 50%, frozen bootstrap 15%다. 과거 풀은 최근 8개가 아니라 전체 이력을 최대 32개로 층화 표본한다. 상대 가중치는 smoothed learner score `p`에 대해 `max(0.05, (1-p)^1)`이며, 어려운 상대를 더 자주 뽑는다.
- 모든 update 뒤 `latest.pt`와 `latest-model.pt`를 원자적으로 저장한다. `model.pt`는 정상 종료 시 저장하고 10 update마다 progress/inference snapshot을 함께 남긴다.
- 평가 로그는 승패와 공격 외에 cancellation, 평균 최대 높이, 구멍, pending/ready garbage, 높이 16 이상 danger rate를 포함한다. champion은 단일 공격량이나 최신 update가 아니라 좌우 교대 held-out 대국과 고정 reference/과거 상대에 대한 결과로 고른다.

모델은 14,883 parameter로 float32 weight만 약 58 KiB다. r1 checkpoint는 legacy actor가 새 76/122-width 관측의 앞 20/12개를 읽도록 호환 로더를 유지하므로 사후 비교는 계속 가능하다. r1 actor를 r2 초기값으로 직접 불러오는 것은 구조가 다르므로 금지하고, 검증된 솔로 bootstrap에서 새로 시작한다.

## 확정 설계

- 정책 행동은 `hold + reachable locked afterstate`이며 프레임 입력은 학습하지 않는다.
- `BattleSession::step_placements`가 양쪽 선택을 같은 12 frame cadence로 동시 적용한다.
- 도달 좌표가 마지막 회전 경로를 허용하면 deterministic planner가 회전 provenance를 보존하여 spin 판정을 잃지 않는다.
- actor는 승격된 2,817-parameter 솔로 scorer를 그대로 포함하고 zero-initialized versus context branch를 더한다.
- critic은 player pair difference에 odd construction을 적용하여 `V(swap(s))=-V(s)`를 만족한다.
- terminal objective는 `win=1, loss=-1, draw=0`인 zero-sum 결과다.
- dense reward는 `0.1 * (0.997*Phi(s') - Phi(s))`만 허용하며 `Phi(terminal)=0`, `Phi(swap)=-Phi(s)`, `|Phi|<=1`이다.
- 상대 풀은 current 50%, historical 30%, frozen bootstrap 20%다.
- 한 match의 상대 종류·historical checkpoint·learner side는 시작 시 seed로 정하고 terminal까지 고정한다. 이 배정도 progress checkpoint에 저장하므로 update 경계나 resume가 게임 의미를 바꾸지 않는다.
- 진행 중인 경기는 update 경계를 넘어 유지한다. progress checkpoint는 각 현재 경기의 seed와 양쪽 action-index 이력을 저장하고, resume 시 Rust가 후보를 다시 열거해 이력을 재연하여 정확한 전투 상태를 복구한다. rollout 경계의 advantage는 현재 value로 bootstrap한다.
- v1의 raw entropy `0.01`은 후보 수가 다른 action set에서 정책을 과도하게 평탄화했다. v2는 `H(pi)/log(N)`을 사용하고 계수를 `0.0005→0.00005`로 100 update 동안 감소시킨다.
- r1은 r0 update 50의 standalone actor를 warm-start로 사용한다. optimizer·환경·상대 배정은 새로 만들며 초기 checkpoint SHA-256을 산출물에 기록한다.
- Rust 후보 diagnostics는 학습 입력이 아니라 선택된 착지의 lines, T-spin class, perfect clear, total attack, cancellation 후 outgoing attack을 기록하는 관측 전용 5정수 buffer다.
- 자원 프로필은 Torch/Rayon thread만 바꾸며 committed semantic config를 바꾸지 않는다.

## 구현 경계

- Rust: `engine-core` direct-afterstate lock boundary, `versus` fixed-cadence battle transition, `arena` parallel versus batch, `py-bridge` zero-copy-shaped byte buffers.
- Python: vector environment decoder, self-contained actor-critic checkpoint, PPO/GAE trainer, potential reward, paired side-swapped evaluator.
- 산출물: progress checkpoint, 시작 기준 `reference-model.pt`, standalone inference model, bounded historical snapshot pool, technique/승패 JSON evaluation report.

## r0 진단과 v2 근거

- 같은 32개 고정 상태에서 r0 update 1→70은 raw entropy `1.2090→2.8197`, effective choices `3.35→16.77`, mean max probability `0.486→0.189`였다. 단순히 방문 상태가 어려워진 결과가 아니라 같은 상태에서 분포가 평평해졌다.
- r0 update 10/30/50/70의 4 seed×좌우×300수 비교와 update 50의 독립 8 seed×좌우×300수 확인 평가를 수행했다. update 50 확인값은 4,800수, attack/piece `0.1054`, outgoing/piece `0.0946`, Tetris/100 `0.375`였다.
- horizon 300과 1,000 모두 초기 후보 대국은 종료되지 않아 승률은 식별력이 없었다. 따라서 이 실험에서 update 50 선택 근거는 공격 효율의 재현과 update 70보다 낮은 entropy의 절충이며, 최종 champion 근거가 아니다.
- v2 로그는 rollout/normalized entropy, effective choices, max probability, entropy objective contribution, approximate KL, clip fraction, gradient norm, explained variance, technique rates와 고정 상대별 rolling score를 함께 기록한다.

## 완료 판정

구현 완료는 학습기 배선 완료를 뜻하며 강한 최종 champion이 이미 만들어졌다는 뜻은 아니다. 실제 champion 판정은 장기 run 뒤 paired held-out matches와 8/12/15 cadence sensitivity 결과로 별도 수행한다.
