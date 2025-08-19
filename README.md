# RTL-rewriter
- This is a RL-based egg rewriter agent for RTL
  
## Instructions for eggenv
- Use [egg](https://github.com/egraphs-good/egg) to build an e-graph and apply equality rewriting rules (rules can be added or modified).
- Use `AstDepth` as the cost function (later can be replaced with XGBoost).
- Provide the following environment interfaces:
  - `reset(expr: str)` → Initialize the environment with an S-expression, return the initial cost.
  - `step(action: int)` → Apply the specified rule, return `(new_cost, reward, best_expr_string)`.
  - `num_actions()` → Return the number of rules (i.e., the size of the action space).
- The reward function is defined as:  
  `reward = old_cost - new_cost`
- Installation:  
  ```bash
  pip install e_rewriter-0.1.0-cp38-cp38-linux_x86_64.whl
## Simply test the basic function
- You only need to run `python test_run.py` ;
