# The base class `object` comes after all other bases at every level, as in the C3 order.

# The shape of `argparse.ArgumentParser`: the first base has no `__init__`, the second base has one.
# Both bases name `object`. The first base must not reach `object.__init__` before the second base.
class Holder(object):
    pass


class Container(object):
    def __init__(self):
        pass


class Parser(Holder, Container):
    def __init__(self):
        super(Parser, self).__init__()
        #                   ^ defined: 10
        super().__init__()
        #       ^ defined: 10


# The same shape with three levels. The first base reaches `object` through its own base.
class Deep(Holder):
    pass


class DeepParser(Deep, Container):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


# A subscripted generic base. The call of `super()` follows it, as `Base` has the `__init__`.
class Base(object):
    def __init__(self):
        pass


class Box(Base[int]):
    def __init__(self):
        super().__init__()
        #       ^ defined: 35


# A subscripted generic first base without `__init__`, and a second base with it.
class Empty(object):
    pass


class Mixed(Empty[int], Container):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


# A subscripted generic second base. The first base has the member.
class Both(Base, Empty[int]):
    def __init__(self):
        super().__init__()
        #       ^ defined: 35


# An external class as the first base. The search reaches the second base.
class External(Thread, Container):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


# A diamond. The left side reaches `object` through the root. The right side has the `__init__`.
class Root(object):
    pass


class Left(Root):
    pass


class Right(Root):
    def __init__(self):
        pass


class Bottom(Left, Right):
    def __init__(self):
        super().__init__()
        #       ^ defined: 80


# A diamond where the root has the member. The left side reaches it first (depth first).
class Root2(object):
    def __init__(self):
        pass


class Left2(Root2):
    pass


class Right2(Root2):
    pass


class Bottom2(Left2, Right2):
    def __init__(self):
        super().__init__()
        #       ^ defined: 92


# A single base.
class Single(Container):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


# A single base without `__init__`: no definition in this file.
class SingleEmpty(Holder):
    def __init__(self):
        super().__init__()
        #       ^ defined:


# The base `object` is last in the base list.
class Last(Container, object):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


class First(object, Container):
    def __init__(self):
        super().__init__()
        #       ^ defined: 10


# A member that the base `object` provides does not shadow a member of a later base.
class WithRun(object):
    pass


class RunBase(object):
    def run(self):
        pass


class Runner(WithRun, RunBase):
    def go(self):
        self.run()
        #    ^ defined: 143
