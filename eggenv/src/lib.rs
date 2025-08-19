use egg::*;
use pyo3::prelude::*;

/* 1) 语言定义：显式支持 0/1 作为常量；+ * ! -> 四个布尔算子 */
define_language! {
    enum Prop {
        "0" = Zero,
        "1" = One,

        "!" = Not(Id),
        "+" = Or([Id; 2]),
        "*" = And([Id; 2]),
        "->" = Implies([Id; 2]),
        "let" = Let([Id; 2]),
        "&" = Concat([Id; 2]),
        Symbol(egg::Symbol),
    }
}

/* 2) 常量折叠：把纯常量子式直接归约为 0/1 */
#[derive(Default, Clone)]
struct ConstantFold;

impl Analysis<Prop> for ConstantFold {
    // Data = Option<(bool, PatternAst<Prop>)>
    //   bool: 具体的值 (false=0, true=1)
    //   PatternAst<Prop>: 代表一个等价的 AST
    type Data = Option<(bool, PatternAst<Prop>)>;

    fn merge(&mut self, to: &mut Self::Data, from: Self::Data) -> DidMerge {
        merge_option(to, from, |a, b| {
            assert_eq!(a.0, b.0, "Merged non-equal constants");
            DidMerge(false, false)
        })
    }

    fn make(egraph: &EGraph<Prop, ConstantFold>, enode: &Prop) -> Self::Data {
        let x = |i: &Id| egraph[*i].data.as_ref().map(|c| c.0);

        let result = match enode {
            // 常量 0/1
            Prop::Zero => Some((false, "0".parse().unwrap())),
            Prop::One  => Some((true, "1".parse().unwrap())),

            // 逻辑运算
            Prop::Not(a) => Some((!x(a)?, format!("(! {})", x(a)? as u8).parse().unwrap())),
            Prop::Or([a, b]) => Some((
                x(a)? || x(b)?,
                format!("(+ {} {})", x(a)? as u8, x(b)? as u8).parse().unwrap(),
            )),
            Prop::And([a, b]) => Some((
                x(a)? && x(b)?,
                format!("(* {} {})", x(a)? as u8, x(b)? as u8).parse().unwrap(),
            )),
            Prop::Implies([a, b]) => Some((
                !x(a)? || x(b)?,
                format!("(-> {} {})", x(a)? as u8, x(b)? as u8).parse().unwrap(),
            )),
            Prop::Concat([a, b]) => Some((
                x(a)? & x(b)?,
                format!("(& {} {})", x(a)? as u8, x(b)? as u8).parse().unwrap(),
            )),

            // let 只是示意（你可能不需要）
            Prop::Let([a, b]) => Some((
                x(a) == x(b),
                format!("(let {} {})", x(a)? as u8, x(b)? as u8).parse().unwrap(),
            )),

            // 变量 / 符号 => 无法折叠
            Prop::Symbol(_) => None,
        };

        result
    }

    fn modify(egraph: &mut EGraph<Prop, ConstantFold>, id: Id) {
        if let Some((val, pat)) = egraph[id].data.clone() {
            // 映射 bool -> "0"/"1"
            let val_str = if val { "1" } else { "0" };
            egraph.union_instantiations(
                &pat,
                &val_str.parse().unwrap(),
                &Default::default(),
                "const-fold".to_string(),
            );
        }
    }
}
/* 3) 代价函数：节点数 */
struct AstCost;
impl CostFunction<Prop> for AstCost {
    type Cost = usize;
    fn cost<C>(&mut self, enode: &Prop, mut child: C) -> Self::Cost
    where
        C: FnMut(Id) -> Self::Cost,
    {
        1 + enode.children().iter().copied().map(&mut child).sum::<usize>()
    }
}

/* 4) 你的两组规则整合 */
fn make_rules_enhance() -> Vec<Rewrite<Prop, ConstantFold>> {
    let mut rws: Vec<Rewrite<Prop, ConstantFold>> = vec![
        // Boolean theorems of one variable
        rewrite!("null-element1"; "(* ?b 0)" => "0"),
        rewrite!("null-element2"; "(+ ?b 1)" => "1"),
        rewrite!("complements1"; "(* ?b (! ?b))" => "0"),
        rewrite!("complements2"; "(+ ?b (! ?b))" => "1"),
        rewrite!("covering1"; "(* ?b (+ ?b ?c))" => "?b"),
        rewrite!("covering2"; "(+ ?b (* ?b ?c))" => "?b"),
        rewrite!("combining1"; "(+ (* ?b ?c) (* ?b (! ?c)))" => "?b"),
        rewrite!("combining2"; "(* (+ ?b ?c) (+ ?b (! ?c)))" => "?b"),
    ];

    rws.extend(rewrite!("identity1"; "(* ?b 1)" <=> "?b"));
    rws.extend(rewrite!("identity2"; "(+ ?b 0)" <=> "?b"));
    rws.extend(rewrite!("idempotency1"; "(* ?b ?b)" <=> "?b"));
    rws.extend(rewrite!("idempotency2"; "(+ ?b ?b)" <=> "?b"));
    rws.extend(rewrite!("involution1"; "(! (! ?b))" <=> "?b"));
    rws.extend(rewrite!("commutativity1"; "(* ?b ?c)" <=> "(* ?c ?b)"));
    rws.extend(rewrite!("commutativity2"; "(+ ?b ?c)" <=> "(+ ?c ?b)"));
    rws.extend(rewrite!("associativity1"; "(* (* ?b ?c) ?d)" <=> "(* ?b (* ?c ?d))"));
    rws.extend(rewrite!("associativity2"; "(+ (+ ?b ?c) ?d)" <=> "(+ ?b (+ ?c ?d))"));
    rws.extend(rewrite!("distributivity1"; "(+ (* ?b ?c) (* ?b ?d))" <=> "(* ?b (+ ?c ?d))"));
    rws.extend(rewrite!("distributivity2"; "(* (+ ?b ?c) (+ ?b ?d))" <=> "(+ ?b (* ?c ?d))"));
    rws.extend(rewrite!("consensus1";
        "(+ (+ (* ?b ?c) (* (! ?b) ?d)) (* ?c ?d))"
        <=>
        "(+ (* ?b ?c) (* (! ?b) ?d))"
    ));
    rws.extend(rewrite!("consensus2";
        "(* (* (+ ?b ?c) (+ (! ?b) ?d)) (+ ?c ?d))"
        <=>
        "(* (+ ?b ?c) (+ (! ?b) ?d))"
    ));
    rws.extend(rewrite!("de-morgan1"; "(! (* ?b ?c))" <=> "(+ (! ?b) (! ?c))"));
    rws.extend(rewrite!("de-morgan2"; "(! (+ ?b ?c))" <=> "(* (! ?b) (! ?c))"));

    rws
}

fn make_rules_basic() -> Vec<Rewrite<Prop, ConstantFold>> {
    vec![
        rewrite!("th1"; "(-> ?x ?y)" => "(+ (! ?x) ?y)"),
        rewrite!("th2"; "(! (! ?x))" => "?x"),
        rewrite!("th3"; "(+ ?x (+ ?y ?z))" => "(+ (+ ?x ?y) ?z)"),
        rewrite!("th4"; "(* ?x (+ ?y ?z))" => "(+ (* ?x ?y) (* ?x ?z))"),
        rewrite!("th5"; "(+ ?x (* ?y ?z))" => "(* (+ ?x ?y) (+ ?x ?z))"),
        rewrite!("th6"; "(+ ?x ?y)" => "(+ ?y ?x)"),
        rewrite!("th7"; "(* ?x ?y)" => "(* ?y ?x)"),
        rewrite!("th9"; "(-> ?x ?y)" => "(-> (! ?y) (! ?x))"),
        rewrite!("th10"; "(+ ?x (* ?x ?y))" => "?x"),
        rewrite!("th11"; "(+ ?x (* (! ?x) ?y))" => "(+ ?x ?y)"),
        rewrite!("th12";
            "(+ (* ?x ?y) (+ (* (! ?x) ?z) (* ?y ?z)))"
            =>
            "(+ (* ?x ?y) (* (! ?x) ?z))"
        ),
        rewrite!("th13"; "(* ?x (+ ?x ?y))" => "?x"),
        rewrite!("th14"; "(* ?x (+ (! ?x) ?y))" => "(* ?x ?y)"),
        rewrite!("th15"; "(* (+ ?x ?y) (+ ?x (! ?y)))" => "?x"),
        rewrite!("th16"; "(* (+ ?x ?y) (+ (! ?x) ?z))" => "(+ (* ?x ?z) (* (! ?x) ?y))"),
        rewrite!("th17";
            "(* (+ ?x ?y) (* (+ (! ?x) ?z) (+ ?y ?z)))"
            =>
            "(* (+ ?x ?y) (+ (! ?x) ?z))"
        ),
    ]
}

fn make_all_rules() -> Vec<Rewrite<Prop, ConstantFold>> {
    let mut v = make_rules_enhance();
    v.extend(make_rules_basic());
    v
}

/* 5) 暴露给 Python 的环境 */
#[pyclass]
struct EggEnv {
    rules: Vec<Rewrite<Prop, ConstantFold>>,
    egraph: EGraph<Prop, ConstantFold>,
    root: Id,
    best_cost: usize,
    init_expr: String,
}

#[pymethods]
impl EggEnv {
    #[new]
    pub fn new(expr_str: String) -> PyResult<Self> {
        let expr: RecExpr<Prop> = expr_str
            .parse()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("parse sexpr failed"))?;

        let mut egraph: EGraph<Prop, ConstantFold> = EGraph::default();
        let root = egraph.add_expr(&expr);

        let extractor = Extractor::new(&egraph, AstCost);
        let (c, _) = extractor.find_best(root);

        Ok(Self {
            rules: make_all_rules(),
            egraph,
            root,
            best_cost: c,
            init_expr: expr_str,
        })
    }

    pub fn num_actions(&self) -> usize { self.rules.len() }

    /// step(action) -> (new_cost, reward, best_expr_string)
    pub fn step(&mut self, action: usize) -> PyResult<(usize, f64, String)> {
        if action >= self.rules.len() {
            return Err(pyo3::exceptions::PyValueError::new_err("invalid action"));
        }

        let ext = Extractor::new(&self.egraph, AstCost);
        let (old_cost, best_expr) = ext.find_best(self.root);

        let rule = self.rules[action].clone();
        let runner = Runner::<Prop, ConstantFold>::default()
            .with_egraph(self.egraph.clone())
            .with_expr(&best_expr)
            .with_iter_limit(1)
            .run(&[rule]);

        self.egraph = runner.egraph;
        self.root = runner.roots[0];

        let ext2 = Extractor::new(&self.egraph, AstCost);
        let (new_cost, new_best) = ext2.find_best(self.root);
        let reward = if new_cost < old_cost { (old_cost - new_cost) as f64 } else { 0.0 };
        if new_cost < self.best_cost { self.best_cost = new_cost; }

        Ok((new_cost, reward, new_best.to_string()))
    }

    /// reset(Some(expr)) 重置为新表达式；reset(None) 复用当前最优表达式
    pub fn reset(&mut self, maybe_expr: Option<String>) -> PyResult<usize> {
        if let Some(s) = maybe_expr {
            let expr: RecExpr<Prop> = s
                .parse()
                .map_err(|_| pyo3::exceptions::PyValueError::new_err("parse sexpr failed"))?;
            self.egraph = EGraph::<Prop, ConstantFold>::default();
            self.root = self.egraph.add_expr(&expr);
            self.init_expr = s;
        } else {
            let ext = Extractor::new(&self.egraph, AstCost);
            let (_c, best_expr) = ext.find_best(self.root);
            self.egraph = EGraph::<Prop, ConstantFold>::default();
            self.root = self.egraph.add_expr(&best_expr);
        }
        let ext = Extractor::new(&self.egraph, AstCost);
        let (c, _) = ext.find_best(self.root);
        self.best_cost = c;
        Ok(c)
    }

    /// 方便 Python 打印初始表达式
    #[getter]
    pub fn reset_expr(&self) -> String {
        self.init_expr.clone()
    }
}

/* 6) pyo3 模块导出 */
#[pymodule]
fn eggenv(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<EggEnv>()?;
    Ok(())
}