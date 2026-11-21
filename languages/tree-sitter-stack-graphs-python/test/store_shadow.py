# A store `o.x = x` has the name of the field on both sides. The route of the right hand side
# reaches the field of the class by the lexical scopes. The stitcher must not drop the route of
# the return annotation for that reason.

class A:
    x: int

    def copy(self) -> A:
        return A()

    def one(self, x):
        o = self.copy()
        o.x = x
        # ^ defined: 6, 11, 13

    def two(self, y):
        o = self.copy()
        o.x = y
        # ^ defined: 6, 16, 18
