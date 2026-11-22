# In a function body, `x: Foo = value` narrows the variable to the value at a read after the
# assignment. The annotation still gives the class when the value reaches no class, and for a
# variable that the statement does not narrow: an attribute, a variable of a module or a class.

class Base:
    def m(self):
        pass

class Derived(Base):
    def m(self):
        pass

def local_variable():
    # The value wins over the annotation at a read, as the narrowing of an assignment.
    x: Base = Derived()
    x.m
    # ^ defined: 10

def parameter(y: Base):
    # A parameter has no value. The annotation gives the class.
    y.m
    # ^ defined: 6

def no_class_value():
    # A value that reaches no class leaves the annotation.
    z: Base = unknown()
    z.m
    # ^ defined: 6

def no_value():
    w: Base
    w.m
    # ^ defined: 6

# A module variable keeps the annotation first. A reader can see it from any function.
module_var: Base = Derived()
module_var.m
#          ^ defined: 6, 10

class Holder:
    # A class variable keeps the annotation first.
    class_var: Base = Derived()

    def __init__(self):
        # An attribute keeps the annotation first.
        self.attr: Base = Derived()

    def read(self):
        self.attr.m
        #         ^ defined: 6, 10

Holder.class_var.m
#                ^ defined: 6, 10
