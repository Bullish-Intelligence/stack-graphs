# Only a method that can store an attribute of its first parameter joins the member attributes of
# its class. These tests keep every form of a store. The rules define no member for the target of
# a `with` clause. A method that stores nothing adds no definitions.

class Store:
    def plain(self):
        self.a = 1

    def augmented(self):
        self.b += 1

    def annotated(self):
        self.c: int = 0

    def pair(self):
        self.d, self.e = 1, 2

    def bracketed(self):
        (self.f, self.g) = 1, 2

    def loop(self):
        for self.h in range(3):
            pass

    def context(self):
        with open("x") as self.i:
            pass

    def spaced(self):
        self . j = 1

    def other_name(me):
        me.k = 1

    def chained(self):
        self.a.l = 1

    @property
    def decorated(self):
        self.m = 1
        return self.m

    @classmethod
    def make(cls):
        cls.n = 1

    def reader(self):
        return self.a

    def silent(self):
        return 1

    def stub(self): ...

s = Store()
s.a
# ^ defined: 7
s.b
# ^ defined: 10
s.c
# ^ defined: 13
s.d
# ^ defined: 16
s.e
# ^ defined: 16
s.f
# ^ defined: 19
s.g
# ^ defined: 19
s.h
# ^ defined: 22
s.j
# ^ defined: 30
s.k
# ^ defined: 33
s.m
# ^ defined: 40
Store.n
#     ^ defined: 45
s.reader
#  ^ defined: 47
s.silent
#  ^ defined: 50
s.stub
#  ^ defined: 53
