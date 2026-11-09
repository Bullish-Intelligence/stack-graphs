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
