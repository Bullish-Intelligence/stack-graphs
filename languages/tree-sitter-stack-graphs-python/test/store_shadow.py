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

    def three(self, x):
        self.x = x
        #    ^ defined: 6, 21, 22
        #        ^ defined: 21

# Method `one` fails when the stitcher ignores the syntax type "store" of the start node: the
# field on line 6 is lost. Methods `two` and `three` give the same result with and without the
# flag. They pin the other forms of a store.
