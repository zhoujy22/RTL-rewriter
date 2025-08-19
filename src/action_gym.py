import gym
import numpy as np
import eggenv 

class EggGymEnv(gym.Env):
    def __init__(self, expr_str):
        super().__init__()
        self.expr_str = expr_str
        self.env = eggenv.EggEnv(expr_str)
        self.action_space = gym.spaces.Discrete(self.env.num_actions())
        self.observation_space = gym.spaces.Box(low=0, high=1e6, shape=(1,), dtype=np.float32)
        self.state = None

    def reset(self):
        cost = self.env.reset(self.expr_str)
        self.state = np.array([cost], dtype=np.float32)
        return self.state

    def step(self, action):
        cost, reward, expr= self.env.step(int(action))
        self.state = np.array([cost], dtype=np.float32)
        done = (reward == 0.0) or (cost <= 1)
        return self.state, reward, done, {"expr": expr}