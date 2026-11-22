# The `self` of a store in a tuple or a list target resolves like the `self` of a single store.
# The attribute names of the targets define members.

class Pair:
    def single(self):
        self.s = 1
#       ^ defined: 5

    def tuple_target(self):
        self.a, self.b = 1, 2
#       ^ defined: 9
#               ^ defined: 9

    def parenthesized(self):
        (self.c, self.d) = 1, 2
#        ^ defined: 14
#                ^ defined: 14

    def bracketed(self):
        [self.e, self.f] = 1, 2
#        ^ defined: 19
#                ^ defined: 19

    def mixed(self):
        first, self.g = 1, 2
#              ^ defined: 24

    def nested(self):
        (self.h, (self.i, other)) = (1, (2, 3))
#        ^ defined: 28
#                 ^ defined: 28

    def loop(self, items):
        for i, self.k in items:
#              ^ defined: 33
            pass

    def subscript(self, items, index):
        items[index], self.m = 1, 2
#                     ^ defined: 38

    def read_only(self):
        return self.s
#              ^ defined: 42

p = Pair()
p.s
# ^ defined: 6
p.a
# ^ defined: 10
p.b
# ^ defined: 10
p.c
# ^ defined: 15
p.d
# ^ defined: 15
p.e
# ^ defined: 20
p.f
# ^ defined: 20
p.g
# ^ defined: 25
p.h
# ^ defined: 29
p.i
# ^ defined: 29
p.k
# ^ defined: 34
p.m
# ^ defined: 39

