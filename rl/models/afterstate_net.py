import torch
import torch.nn as nn
import torch.nn.functional as F

class AfterstateValueNet(nn.Module):
    """
    Deep Neural Evaluator for Tetris Afterstate Positions s'.
    Inputs:
        board_tensor: (Batch, 4, 20, 10) - Player/Opponent Board & Garbage grids
        meta_vector:  (Batch, 64)        - B2B, Combo, Queue, Opponent meta features
    Outputs:
        value:        (Batch, 1)         - State Value V(s') in range [-10.0, +10.0]
    """
    def __init__(self):
        super().__init__()
        # Conv Spatial Backbone
        self.conv1 = nn.Conv2d(4, 32, kernel_size=3, padding=1)
        self.bn1 = nn.BatchNorm2d(32)
        self.conv2 = nn.Conv2d(32, 64, kernel_size=3, padding=1)
        self.bn2 = nn.BatchNorm2d(64)
        self.conv3 = nn.Conv2d(64, 64, kernel_size=3, padding=1)
        self.bn3 = nn.BatchNorm2d(64)

        self.spatial_fc = nn.Linear(64 * 20 * 10, 256)

        # Meta Feature Encoder
        self.meta_fc = nn.Linear(64, 128)

        # Combined Value Head
        self.fc_combined = nn.Linear(256 + 128, 256)
        self.value_head = nn.Linear(256, 1)

    def forward(self, board_tensor, meta_vector):
        x = F.relu(self.bn1(self.conv1(board_tensor)))
        x = F.relu(self.bn2(self.conv2(x)))
        x = F.relu(self.bn3(self.conv3(x)))

        x_flat = x.view(x.size(0), -1)
        spatial_feat = F.relu(self.spatial_fc(x_flat))

        meta_feat = F.relu(self.meta_fc(meta_vector))

        combined = torch.cat([spatial_feat, meta_feat], dim=1)
        h = F.relu(self.fc_combined(combined))
        value = self.value_head(h)

        return value
