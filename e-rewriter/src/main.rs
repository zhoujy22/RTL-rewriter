// src/main.rs
use std::{env, fs, io::{self, Read}};
use egg::{*, SymbolLang as L};

/// 生成重写规则（保持你原来的结构与名字）
fn make_rules() -> Vec<Rewrite<L, ()>> {
    // 便捷宏
    macro_rules! rw {
        ($n:expr; $l:expr => $r:expr) => {{
            let l: Pattern<L> = $l.parse().unwrap();
            let r: Pattern<L> = $r.parse().unwrap();
            Rewrite::new($n, l, r).unwrap()
        }};
    }

    vec![
        rw!("and-commute"; "(And ?a ?b)" => "(And ?b ?a)"),
        rw!("or-commute";  "(Or ?a ?b)"  => "(Or ?b ?a)"),
        rw!("and-id";      "(And ?a true)" => "?a"),
        rw!("or-id";       "(Or ?a false)" => "?a"),
        rw!("if-true";     "(If true  ?a ?b)" => "?a"),
        rw!("if-false";    "(If false ?a ?b)" => "?b"),
        rw!("if-same";     "(If ?cond ?a ?a)" => "?a"),
        rw!("add-zero";    "(Add ?a 0)" => "?a"),
        rw!("add-zero-comm"; "(Add 0 ?a)" => "?a"),
        rw!("mul-one";     "(Mul ?a 1)" => "?a"),
        rw!("mul-one-comm"; "(Mul 1 ?a)" => "?a"),
        rw!("mul-zero";    "(Mul ?a 0)" => "0"),
        rw!("mul-zero-comm"; "(Mul 0 ?a)" => "0"),

        // Case <-> If（两分支 + default）
        rw!(
            "case2-to-if-else";
            "(Case ?sel 
                (TpCaseList 
                    (TpCase (TpCaseLblList (CaseLbl ?l1)) ?v1)
                    (TpCase (TpCaseLblList default) ?v2)
                )
            )"
            =>
            "(If (Eq ?sel ?l1) ?v1 ?v2)"
        ),
        rw!(
            "if-to-case-loose";
            "(If (Eq ?sel ?label) ?then ?else)"
            =>
            "(Case ?sel
                (TpCaseList
                    (TpCase (TpCaseLblList (CaseLbl ?label)) ?then)
                    (TpCase (TpCaseLblList default) ?else)))"
        ),
    ]
}

/// 与你原来的 AstDepth 等价：AST大小 + Case 惩罚
struct AstDepth;
impl CostFunction<L> for AstDepth {
    type Cost = usize;
    fn cost<C>(&mut self, enode: &L, mut child: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        let base = 1 + enode.children().iter().copied().map(&mut child).sum::<usize>();
        let penalty = if enode.op.as_str() == "Case" { 100 } else { 0 };
        base + penalty
    }
}

fn main() -> anyhow::Result<()> {
    // 1) 读文件或 stdin
    let path = "input.sexpr";  // 你的 S-expression 文件路径
    let expr_str = fs::read_to_string(path)
        .expect("无法读取输入文件");

    // 2) 解析为 RecExpr<SymbolLang>（前缀 S 表达式）
    let expr: RecExpr<L> = expr_str.parse().expect("表达式解析失败，请检查语法");
    println!("解析成功的表达式:\n{:#?}", expr);

    // 3) 规则
    let rules = make_rules();

    // 4) 跑 e-graph
    // ===== RL-guided runner: epsilon-greedy bandit over rewrites =====
    #[derive(Clone, Debug)]
    struct RuleArm {
        q: f64,      // 价值估计（平均奖励）
        n: usize,    // 被选择次数
        name: String // 仅调试打印
    }

    // 选择若干规则（子集）——epsilon-greedy：ε 概率随机，1-ε 概率按 q 值贪婪
    fn select_rule_subset(arms: &Vec<RuleArm>, k: usize, epsilon: f64) -> Vec<usize> {
        use rand::{seq::SliceRandom, Rng};
        let mut idxs: Vec<usize> = (0..arms.len()).collect();
        let mut rng = rand::thread_rng();

        if rng.gen::<f64>() < epsilon {
            idxs.shuffle(&mut rng);
            idxs.truncate(k.min(idxs.len()));
            return idxs;
        }
        // 按 q 值排序取前 k 个
        idxs.sort_by(|&i, &j| arms[j].q.partial_cmp(&arms[i].q).unwrap());
        idxs.truncate(k.min(idxs.len()));
        idxs
    }

    // 按选择的索引取子规则向量
    fn gather_rules<L>(
        rules: &Vec<Rewrite<L, ()>>,
        chosen: &Vec<usize>
    ) -> Vec<Rewrite<L, ()>>
    where L: egg::Language + std::fmt::Display + FromOp + 'static
    {
        chosen.iter().map(|&i| rules[i].clone()).collect()
    }

    // 计算奖励：越小的代价越好，用 “旧代价 - 新代价”，且不小于 0
    fn reward(old_cost: usize, new_cost: usize) -> f64 {
        if new_cost < old_cost { (old_cost - new_cost) as f64 } else { 0.0 }
    }

    let mut all_rules = make_rules();
    let mut arms: Vec<RuleArm> = all_rules.iter().map(|r| RuleArm {
        q: 0.0, n: 0, name: r.name.to_string()
    }).collect();

    // 初始：用 0 次迭代建个 e-graph（或 1 次空跑），拿到起始代价
    let mut runner = Runner::<L, ()>::default()
        .with_expr(&expr)
        .with_node_limit(200_000)
        .with_iter_limit(0)   // 不做任何 rewrite，只是建起点
        .run(&[]);

    let mut extractor = Extractor::new(&runner.egraph, AstDepth);
    let mut best = extractor.find_best(runner.roots[0]);
    let mut best_cost = best.0;
    let mut best_expr = best.1.clone();

    let max_steps = 200;        // 总步数上限（相当于 RL 里 T）
    let subset_k = 4;           // 每步选择的规则子集大小（越小越“半饱和”）
    let epsilon = 0.2;          // ε-greedy 探索率
    let patience = 30;          // 连续无改进步数阈值，早停
    let mut no_improve = 0;

    for step in 0..max_steps {
        // 1) 基于 bandit 选择规则子集
        let chosen = select_rule_subset(&arms, subset_k, epsilon);
        let chosen_rules = gather_rules(&all_rules, &chosen);

        // 2) 在“当前 e-graph 状态”上仅进行 1 次迭代的小步推进
        //    用 with_egraph 继承上一步的 egraph，避免从头来
        // 先从上一个 runner 的 root 提取一个表达式
        let extractor_tmp = Extractor::new(&runner.egraph, AstDepth);
        if runner.roots.is_empty() {
            eprintln!("⚠️ 没有 root，跳过 step {}", step);
            continue;
        }
        let (_, root_expr) = extractor_tmp.find_best(runner.roots[0]);

        // 再用 root_expr 初始化新的 runner
        runner = Runner::<L, ()>::default()
            .with_egraph(runner.egraph.clone())       // 保留 e-graph 状态
            .with_expr(&root_expr)            // 重新设 root
            .with_node_limit(200_000)
            .with_iter_limit(1)
            .run(&chosen_rules);

        // 3) 评估新代价，作为奖励反馈
        extractor = Extractor::new(&runner.egraph, AstDepth);
        let (cost_now, expr_now) = extractor.find_best(runner.roots[0]);
        let r = reward(best_cost, cost_now);

        // 4) 更新 bandit 价值估计（增量平均）
        for &i in &chosen {
            arms[i].n += 1;
            let n = arms[i].n as f64;
            arms[i].q += (r - arms[i].q) / n;
        }

        // 5) 记录最优 & 早停
        if cost_now < best_cost {
            best_cost = cost_now;
            best_expr = expr_now.clone();
            no_improve = 0;
        } else {
            no_improve += 1;
        }

        // （可选）打印进度
        if step % 10 == 0 {
            eprintln!(
                "Step {:>4} | cost={} | reward={:.1} | best={}",
                step, cost_now, r, best_cost
            );
        }

        if no_improve >= patience {
            eprintln!("Early stop: no improvement in {} steps.", patience);
            break;
        }

        // 超出节点上限也提前停止
        if runner.egraph.total_size() > 190_000 {
            eprintln!("Node near limit, stopping to avoid blow-up.");
            break;
        }
    }

    // 最终结果沿用你原来的写法
    println!("\n✅ 最优表达式（AST代价={}）:\n{}", best_cost, best_expr);
    std::fs::write("/home/jingyi/workspace/E-Syn/test/optimized.sexpr", best_expr.to_string())
            .expect("❌ 写入 optimized.sexpr 失败");

    

    Ok(())
}