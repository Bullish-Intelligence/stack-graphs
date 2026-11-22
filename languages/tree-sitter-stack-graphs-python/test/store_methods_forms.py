# More forms of a store through the first parameter. A method with one of them joins the member
# attributes of its class. A method with a read or a call only does not.
from typing import Annotated

class Forms:
    def annotation_only(self):
        self.a: int

    def target_after_comma(self, items):
        for i, self.b in items:
            pass

    def continued(self):
        self.c \
            = 1

    def annotated_call(self):
        self.d: Annotated[int, Field(default=1)] = 1

    def decorated_annotation(self):
        self.e: int

    @property
    def decorated_comma(self, items):
        for i, self.f in items:
            pass

    @staticmethod
    def static_store(self):
        self.g = 1

    def pair_after_name(self):
        first, self.h = 1, 2

    def read_only(self, items):
        if self.a:
            return self.a
        for i in self.a:
            pass
        for i, j in self.a:
            pass
        x = {self.a: 1}
        self.read_method(key=1)
        return x

    def read_method(self, key):
        return key

f = Forms()
f.a
# ^ defined: 7
f.b
# ^ defined: 10
f.c
# ^ defined: 14
f.d
# ^ defined: 18
f.e
# ^ defined: 21
f.f
# ^ defined: 25
f.g
# ^ defined:
f.h
# ^ defined: 33
