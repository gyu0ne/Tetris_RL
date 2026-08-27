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
- 진행 중인 경기는 update 경계를 넘어 유지한다. progress checkpoint는 각 현재 경기의 seed와 양쪽 action-index 이력을 저장하고, resume 시 Rust가 후보를 다시 열거해 이력을 재연하여 정확한 전투 상태를 복구한다. rollout 경계의 advantage는 현재 value로 bootstrap한다.
- 자원 프로필은 Torch/Rayon thread만 바꾸며 committed semantic config를 바꾸지 않는다.

## 구현 경계

- Rust: `engine-core` direct-afterstate lock boundary, `versus` fixed-cadence battle transition, `arena` parallel versus batch, `py-bridge` zero-copy-shaped byte buffers.
- Python: vector environment decoder, self-contained actor-critic checkpoint, PPO/GAE trainer, potential reward, paired side-swapped evaluator.
- 산출물: progress checkpoint, standalone inference model, bounded historical snapshot pool, JSON evaluation report.

## 완료 판정

구현 완료는 학습기 배선 완료를 뜻하며 강한 최종 champion이 이미 만들어졌다는 뜻은 아니다. 실제 champion 판정은 장기 run 뒤 paired held-out matches와 8/12/15 cadence sensitivity 결과로 별도 수행한다.
