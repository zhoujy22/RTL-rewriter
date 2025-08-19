import numpy as np
from action_gym import EggGymEnv
from train import train_dqn

def test_simple_expr():
    # 初始化环境，输入 (Add (Mul x 1) 0)
    env = EggGymEnv("(Add (Mul x 1) 0)")

    # 查看动作数量
    print("可用规则数 =", env.action_space.n)

    # reset: 返回初始 cost
    state = env.reset()
    print("初始状态 cost =", state)

    # 随机走一步（测试环境 step）
    action = np.random.randint(env.action_space.n)
    next_state, reward, done, _ = env.step(action)
    print(f"测试环境：执行动作 {action}, next_state={next_state}, reward={reward}")

    # 开始训练 DQN
    print("\n开始 DQN 训练")
    train_dqn(env, episodes=50)

if __name__ == "__main__":
    test_simple_expr()