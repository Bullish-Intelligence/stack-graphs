class Node:
    pass

class SymbolNode(Node):
    def name(self):
        pass

class Var(SymbolNode):
    def name(self):
        pass

class Expr(Node):
    pass

class RefExpr(Expr):
    def __init__(self):
        self.node: SymbolNode | None = None

def f(lvs: list[Expr]):
    for lv in lvs:
        if isinstance(lv, RefExpr) and isinstance(lv.node, Var):
            n = lv.node.name
            #           ^ defined: 9

def g(lv: RefExpr):
    if isinstance(lv.node, Var):
        m = lv.node.name
        #           ^ defined: 9
    # Outside of the guard, the annotation of the attribute decides.
    k = lv.node.name
    #           ^ defined: 5
