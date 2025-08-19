import torch
import torch.nn as nn
import torch.optim as optim
import random
from collections import deque
import numpy as np

class QNet(nn.Module):
    def __init__(self, state_dim, action_dim):
        super().__init__()
        self.fc = nn.Sequential(
            nn.Linear(state_dim, 64), nn.ReLU(),
            nn.Linear(64, action_dim)
        )
    def forward(self, x):
        return self.fc(x)

def train_dqn(env, episodes=100):
    qnet = QNet(env.observation_space.shape[0], env.action_space.n)
    target_qnet = QNet(env.observation_space.shape[0], env.action_space.n)
    target_qnet.load_state_dict(qnet.state_dict())
    opt = optim.Adam(qnet.parameters(), lr=1e-3)
    buffer = deque(maxlen=10000)
    gamma = 0.9
    eps = 0.2

    for ep in range(episodes):
        state = env.reset()
        ep_reward = 0
        print(f"\n=== Episode {ep} ===")
        print("初始表达式:", env.expr_str)
        for t in range(50):
            if random.random() < eps:
                action = env.action_space.sample()
            else:
                with torch.no_grad():
                    action = qnet(torch.tensor(state).float().unsqueeze(0)).argmax().item()

            next_state, reward, done, info = env.step(action)
            buffer.append((state, action, reward, next_state))
            state = next_state
            ep_reward += reward
            print(f" step {t:02d}, action={action}, expr={info['expr']}, cost={next_state[0]}, reward={reward}")

            if len(buffer) > 32:
                batch = random.sample(buffer, 32)
                s, a, r, s2 = zip(*batch)
                s = torch.tensor(np.array(s), dtype=torch.float32)
                a = torch.tensor(a).unsqueeze(1)
                r = torch.tensor(r, dtype=torch.float32)
                s2 = torch.tensor(np.array(s2), dtype=torch.float32)

                qvals = qnet(s).gather(1, a).squeeze()
                next_q = target_qnet(s2).max(1)[0].detach()
                target = r + gamma * next_q
                loss = ((qvals - target)**2).mean()

                opt.zero_grad()
                loss.backward()
                opt.step()
            if done:
                    break
        if ep % 10 == 0:
            target_qnet.load_state_dict(qnet.state_dict())
            print(f"Episode {ep}, total reward {ep_reward}")
