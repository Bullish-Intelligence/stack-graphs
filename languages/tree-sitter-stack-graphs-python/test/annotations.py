# A written annotation makes a name behave like an instance of the annotated class.
# The rules follow the annotation only. They infer no type.

class Foo:
    def bar(self):
        pass

class Other:
    def baz(self):
        pass

# A parameter with an annotation.
def parameter(x: Foo, y: Foo = None, z: "Foo" = None):
    x.bar
    #  ^ defined: 5
    y.bar
    #  ^ defined: 5
    z.bar
    #  ^ defined: 5

# A reference to the name still resolves to the name only.
def plain_name(x: Foo):
    return x
    #      ^ defined: 22

# A variable with an annotation.
variable: Foo = Foo()
variable.bar
#        ^ defined: 5
variable
# ^ defined: 27

declared: Foo
declared.bar
#        ^ defined: 5

# An attribute that a method annotates.
class Holder:
    class_level: Foo
    optional: Foo | None

    def __init__(self):
        self.attribute: Foo = None
        self.either: Other | Foo = None

    def use(self):
        self.class_level.bar
        #                ^ defined: 5
        self.optional.bar
        #             ^ defined: 5
        self.attribute.bar
        #              ^ defined: 5
        self.either.bar
        #           ^ defined: 5
        self.either.baz
        #           ^ defined: 9

# A return annotation.
def make() -> Foo:
    pass

make().bar
#      ^ defined: 5

def make_string() -> "Foo":
    pass

make_string().bar
#             ^ defined: 5

# Optional, Union, and the union written with a bar.
def wrappers(a: Optional[Foo], b: Foo | None, c: Optional["Foo"], d: Union[Foo, Other], e: Foo | Other | None):
    a.bar
    #  ^ defined: 5
    b.bar
    #  ^ defined: 5
    c.bar
    #  ^ defined: 5
    d.bar
    #  ^ defined: 5
    d.baz
    #  ^ defined: 9
    e.bar
    #  ^ defined: 5
    e.baz
    #  ^ defined: 9

# A generic type resolves the container only. The element type does not reach the name.
class Container:
    def put(self):
        pass

class Box(Container):
    pass

def generics(a: Box[Foo], b: Container[Foo]):
    a.put
    #  ^ defined: 90
    b.put
    #  ^ defined: 90
    a.bar
    #  ^ defined:
    b.bar
    #  ^ defined:

# A star parameter holds a tuple, not an instance of the annotated class.
def star(*args: Foo, **kwargs: Foo):
    args.bar
    #    ^ defined:
    kwargs.bar
    #      ^ defined:

# An annotation reaches the members of the base classes of the class.
class Child(Foo):
    pass

def inherited(child: Child):
    child.bar
    #     ^ defined: 5

# Without an annotation, a name has no class.
def unannotated(x):
    x.bar
    #  ^ defined:
