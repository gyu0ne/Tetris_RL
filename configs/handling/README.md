# 플레이어 Handling 레코드

관측 대상 TETRA LEAGUE 프로필은 `room_handling = false`이므로 각 리플레이 픽스처는 당시 플레이어의 유효 handling을 함께 가져야 한다.

버전 1 정규화 레코드는 다음 값을 프레임 단위로 보존한다.

- `schema_version = 1`
- `das_frames`
- `arr_frames`
- `dcd_frames`
- `soft_drop`: `disabled`, 양의 `cells_per_frame`, 또는 `sonic`
- 원본 리플레이 SHA-256, 클라이언트 asset ID, 원본 값과 원본 단위

원본 UI/리플레이 값에서 프레임으로 바꾸는 계산은 클라이언트 버전별 어댑터가 담당한다. 단위를 확인할 수 없는 값을 추측하여 `PlayerHandlingProfile`에 넣지 않는다.
