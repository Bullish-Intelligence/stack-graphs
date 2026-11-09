# Members resolve through any number of base classes.

class A:
    class_attr = 1

    def __init__(self):
        self.instance_attr = 2

    def method(self):
        pass

    def overridden(self):
        pass

class B(A):
    pass

class C(B):
    pass

class D(C):
    pass

# Two levels.
C.method
#  ^ defined: 9
C().method
#    ^ defined: 9
C.class_attr
#  ^ defined: 4
C().class_attr
#    ^ defined: 4
C().instance_attr
#    ^ defined: 7

# Three levels.
D.method
#  ^ defined: 9
D().method
#    ^ defined: 9
D.class_attr
#  ^ defined: 4
D().class_attr
#    ^ defined: 4
D().instance_attr
#    ^ defined: 7

class UsesInherited(D):
    def use(self):
        self.method()
        #    ^ defined: 9
        self.class_attr
        #    ^ defined: 4
        self.instance_attr
        #    ^ defined: 7
        super().method()
        #       ^ defined: 9

# A method in a subclass shadows the method of a base class.
class Override(B):
    def overridden(self):
        pass

    def calls(self):
        self.overridden()
        #    ^ defined: 61

Override().overridden
#          ^ defined: 61
Override.overridden
#        ^ defined: 61

class OverrideChild(Override):
    pass

OverrideChild().overridden
#               ^ defined: 61
OverrideChild().method
#               ^ defined: 9

# The base class defines the member that the subclass does not.
class OnlyBase(A):
    def other(self):
        pass

OnlyBase().overridden
#          ^ defined: 12

# A diamond. The member of the shared base class is found once.
class Left(A):
    pass

class Right(A):
    pass

class Bottom(Left, Right):
    pass

Bottom().method
#        ^ defined: 9
Bottom.class_attr
#      ^ defined: 4
Bottom().instance_attr
#        ^ defined: 7

# In a diamond, a member that both sides define is ambiguous.
class LeftOwn(A):
    def own(self):
        pass

class RightOwn(A):
    def own(self):
        pass

class BottomOwn(LeftOwn, RightOwn):
    pass

BottomOwn().own
#           ^ defined: 108, 112
