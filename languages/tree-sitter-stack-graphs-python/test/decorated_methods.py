class A:
    w = 1

    @property
    def p(self):
        return self.w
#                   ^ defined: 2

    @classmethod
    def c(cls):
        cls.z = 2
        return cls.w, cls().p, cls.p, cls.z
#                  ^ defined: 2
#                           ^ defined: 5
#                                  ^ defined: 5
#                                         ^ defined: 11

    @staticmethod
    def s(x):
        return x.w

    @custom.decorator
    def d(self):
        self.q = 3
        return self.q, self.p
#                   ^ defined: 24
#                           ^ defined: 5

# A read of a property behaves like a call of the method. The result gives the type of the read.
class Leaf:
    def go(self):
        pass

class Holder:
    def __init__(self):
        self.made = Leaf()

    @property
    def annotated(self) -> Leaf:
        pass

    @property
    def returned(self):
        return self.made

    @functools.cached_property
    def cached(self) -> Leaf:
        pass

    @property
    @abstractmethod
    def stacked(self) -> Leaf:
        pass

    @property
    def plain(self):
        return 1

    def use(self):
        self.annotated.go
        #              ^ defined: 31
        self.returned.go
        #             ^ defined: 31
        self.cached.go
        #           ^ defined: 31
        self.stacked.go
        #            ^ defined: 31
        self.plain
        #    ^ defined: 56

def read(holder: Holder):
    holder.annotated.go
    #                ^ defined: 31
    Holder().returned.go
    #                 ^ defined: 31
