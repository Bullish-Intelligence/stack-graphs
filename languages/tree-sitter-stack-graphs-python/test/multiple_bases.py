class A:
    def __init__(self):
        pass

    def run(self):
        pass


class B(A):
    def __init__(self):
        super().__init__()
        #         ^ defined: 2

    def run(self):
        pass


class C:
    def __init__(self):
        pass

    def run(self):
        pass


class D(B, C):
    def go(self):
        self.run()
        #    ^ defined: 14, 22
        super().run()
        #       ^ defined: 14, 22
        super().__init__()
        #       ^ defined: 10, 19


def f(d: D):
    d.run()
    #  ^ defined: 14, 22


# A member that only the second base has still resolves to the second base.
class OnlyC(C):
    def extra(self):
        pass


class E(B, OnlyC):
    def go(self):
        self.extra()
        #    ^ defined: 43
        super().extra()
        #       ^ defined: 43


# The rules return the candidates of all bases. In a diamond, both `A2.m` and `C2.m` are
# candidates. Scippy picks `C2.m`, because the C3 order of Python reads `C2` before `A2`.
class A2:
    def m(self):
        pass


class B2(A2):
    pass


class C2(A2):
    def m(self):
        pass


class D2(B2, C2):
    pass


D2().m
#    ^ defined: 58, 67
