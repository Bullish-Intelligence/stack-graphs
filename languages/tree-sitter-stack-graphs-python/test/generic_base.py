from typing import Generic, TypeVar

T = TypeVar("T")
U = TypeVar("U")


class Base(Generic[T]):
    declared: int
    flag: bool = False

    def run(self):
        pass

    @property
    def system(self) -> str:
        return ""

    def shadowed(self):
        pass


# A member of a generic base class, `Base[int]`, is a member of the class `Base`.
class Child(Base[int]):
    def shadowed(self):
        pass

    def go(self):
        self.run()
        #    ^ defined: 11
        self.system
        #    ^ defined: 15
        self.declared
        #    ^ defined: 8
        self.flag
        #    ^ defined: 9
        self.shadowed()
        #    ^ defined: 24


# The base is generic in two levels.
class Grandchild(Child):
    def go(self):
        self.run()
        #    ^ defined: 11


class Mixed(Generic[U], Base[U]):
    def go(self):
        self.run()
        #    ^ defined: 11


# A receiver that an annotation types reaches the members of the generic base class.
def use(c: Child):
    c.run()
    # ^ defined: 11
    c.system
    # ^ defined: 15


# A dataclass with a generic base class and a typed field.
class Part(Base[T]):
    kind: str = "part"


def use_part(p: Part):
    p.run()
    # ^ defined: 11
    p.kind
    # ^ defined: 63


# A comment after the parenthesis of the method does not hide the first parameter.
class Commented(Base[int]):
    def method(  # noqa: C901
        self,
        value: int,
    ) -> int:
        self.run()
        #    ^ defined: 11
        self.system
        #    ^ defined: 15
        return value

    @property
    def prop(  # noqa: C901
        self,
    ) -> int:
        self.run()
        #    ^ defined: 11
        return 1
