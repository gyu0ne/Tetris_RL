# PROJECT Explain: Versus Self-Play RL

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
