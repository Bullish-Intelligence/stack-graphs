class A:
    k = 0
    def __init__(self):
        self.x = 1
        self.c = A()
        self.x = 5
#            ^ defined: 4, 6

# The name of an assignment target is a reference. It also resolves to itself.
a = A()
a.x = 2
# ^ defined: 4, 6, 11
a.y = 3
# ^ defined: 13
A.k = 4
# ^ defined: 2, 15
a.c.x = 6
# ^ defined: 5
#   ^ defined: 4, 6, 17
