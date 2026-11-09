def f(a, b=2):
    pass

f(a=1)
# ^ defined: 1
f(b=1, a=2)
# ^ defined: 1
#      ^ defined: 1

class C:
    def __init__(self, size):
        pass
    def m(self, k):
        pass

C(size=1)
# ^ defined: 11
C(size=1).m(k=2)
# ^ defined: 11
#           ^ defined: 13

g = f
g(a=1)
# ^ defined: 1

# The first parameter of a classmethod is the class, so a keyword argument of its call names a
# parameter of `__init__`.
class D:
    count = 1

    def __init__(self, size, name=None):
        self.size = size

    def method(self):
        pass

    @classmethod
    def make(cls, n):
        obj = cls(size=n, name="x")
        #         ^ defined: 31
        #                 ^ defined: 31
        obj.method
        #   ^ defined: 34
        obj.size
        #   ^ defined: 31,32
        cls.count
        #   ^ defined: 29

    @classmethod
    def clone(cls):
        return cls(size=1).method()
        #          ^ defined: 31
        #                  ^ defined: 34

# The methods `__new__` and `__init_subclass__` take the class, with no decorator.
class E(D):
    def __new__(cls, size):
        return cls(size=size)
        #          ^ defined: 31

    def __init_subclass__(cls):
        cls(size=1)
        #   ^ defined: 31

