# A tuple assignment of TypeVars. The call decides, not the form of the target.
from typing import TypeVar

class Foo:
    def method(self):
        pass

K, V = TypeVar("K"), TypeVar("V")

def f(x: K) -> V:
    return Foo()
f(1).method
#    ^ defined: 5
