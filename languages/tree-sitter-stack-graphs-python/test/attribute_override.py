# An attribute that several classes of a chain store. The lookup of a member of an instance
# resolves to the store of the most derived class. An annotated declaration in the chain wins.

# Two levels. Both classes store `t`.
class Base2:
    def __init__(self):
        self.t = None


class Mid2(Base2):
    def __init__(self):
        super().__init__()
        self.t = 1


def two_mid(x: Mid2):
    return x.t
    #        ^ defined: 13


def two_base(x: Base2):
    return x.t
    #        ^ defined: 7


# Three levels. The two upper classes store `t`. The lookup on the leaf finds the middle store.
class Base3:
    def __init__(self):
        self.t = None


class Mid3(Base3):
    def __init__(self):
        super().__init__()
        self.t = 1


class Leaf3(Mid3):
    pass


def three_leaf(x: Leaf3):
    return x.t
    #        ^ defined: 35


def three_mid(x: Mid3):
    return x.t
    #        ^ defined: 35


def three_base(x: Base3):
    return x.t
    #        ^ defined: 29


# Three levels. Every class stores `t`.
class Base4:
    def __init__(self):
        self.t = None


class Mid4(Base4):
    def __init__(self):
        self.t = 1


class Leaf4(Mid4):
    def __init__(self):
        self.t = 1


def four_leaf(x: Leaf4):
    return x.t
    #        ^ defined: 70


def four_mid(x: Mid4):
    return x.t
    #        ^ defined: 65


# The base stores `t`. The derived classes do not.
class Base5:
    def __init__(self):
        self.t = None


class Leaf5(Base5):
    pass


def base_only(x: Leaf5):
    return x.t
    #        ^ defined: 86


# Only the derived class stores `t`.
class Base6:
    def __init__(self):
        pass


class Leaf6(Base6):
    def __init__(self):
        self.t = None


def derived_only(x: Leaf6):
    return x.t
    #        ^ defined: 106


# An annotated class body declaration in the base wins over the stores of the derived classes.
class Base7:
    t: int

    def __init__(self):
        pass


class Mid7(Base7):
    def __init__(self):
        self.t = 1


class Leaf7(Mid7):
    pass


def annotated_body(x: Leaf7):
    return x.t
    #        ^ defined: 116


# An annotated class body declaration with a value.
class Base8:
    t: int = 0


class Leaf8(Base8):
    def __init__(self):
        self.t = 1


def annotated_body_value(x: Leaf8):
    return x.t
    #        ^ defined: 138


# An annotated store in the base wins over the plain store of the derived class.
class Base9:
    def __init__(self):
        self.t: int = 0


class Leaf9(Base9):
    def __init__(self):
        self.t = 1


def annotated_base_store(x: Leaf9):
    return x.t
    #        ^ defined: 154


# An annotated store in the derived class is the most derived store too.
class Base10:
    def __init__(self):
        self.t = None


class Leaf10(Base10):
    def __init__(self):
        self.t: int = 1


def annotated_derived_store(x: Leaf10):
    return x.t
    #        ^ defined: 175


# An annotated store in the middle class wins over the plain stores above and below it.
class Base11:
    def __init__(self):
        self.t = None


class Mid11(Base11):
    def __init__(self):
        self.t: int = 1


class Leaf11(Mid11):
    def __init__(self):
        self.t = 1


def annotated_middle_store(x: Leaf11):
    return x.t
    #        ^ defined: 191


# An annotated declaration in the root of a chain of plain stores.
class Base12:
    t: int

    def __init__(self):
        self.t = 0


class Mid12(Base12):
    def __init__(self):
        self.t = 1


class Leaf12(Mid12):
    def __init__(self):
        self.t = 1


def annotated_root(x: Leaf12):
    return x.t
    #        ^ defined: 206


# Annotated declarations in two classes of the chain. The most derived one wins.
class Base13:
    t: int = 0


class Mid13(Base13):
    t: int = 1


class Leaf13(Mid13):
    def __init__(self):
        self.t = 2


def two_annotated(x: Leaf13):
    return x.t
    #        ^ defined: 233


# An annotated store with no value.
class Base14:
    def __init__(self):
        self.t: int


class Leaf14(Base14):
    def __init__(self):
        self.t = 1


def annotated_no_value(x: Leaf14):
    return x.t
    #        ^ defined: 249


# A plain assignment in the class body of the derived class does not win over the annotation.
class Base15:
    t: int


class Leaf15(Base15):
    t = 1


def annotated_over_body(x: Leaf15):
    return x.t
    #        ^ defined: 264


# The annotation of another name does not win. Only the name `t` counts.
class Base16:
    u: int

    def __init__(self):
        self.t = 0


class Leaf16(Base16):
    def __init__(self):
        self.t = 1


def other_name(x: Leaf16):
    return x.t
    #        ^ defined: 286


# An annotation on another object is no declaration of the class.
class Base17:
    def __init__(self, other):
        other.t: int = 0


class Leaf17(Base17):
    def __init__(self):
        self.t = 1


def other_object(x: Leaf17):
    return x.t
    #        ^ defined: 302


# An annotation in an unrelated class does not count.
class Unrelated:
    t: int


class Base18:
    def __init__(self):
        self.t = 0


class Leaf18(Base18):
    def __init__(self):
        self.t = 1


def unrelated(x: Leaf18):
    return x.t
    #        ^ defined: 322


# A lookup on the class object finds the annotated declaration of the base.
class Base19:
    t: int


class Leaf19(Base19):
    t = 1


Leaf19.t
#      ^ defined: 332


# A lookup through `self` in a method of the derived class.
class Base20:
    t: int

    def __init__(self):
        self.t = 0


class Leaf20(Base20):
    def __init__(self):
        self.t = 1

    def get(self):
        return self.t
        #           ^ defined: 345


# Two bases. The annotated declaration in the second base wins over the store in the first.
class Left21:
    def __init__(self):
        self.t = 0


class Right21:
    t: int


class Both21(Left21, Right21):
    pass


def two_bases(x: Both21):
    return x.t
    #        ^ defined: 367


# A method override is no attribute override: the most derived method wins.
class Base22:
    def run(self):
        pass


class Leaf22(Base22):
    def run(self):
        pass


def method_override(x: Leaf22):
    return x.run
    #        ^ defined: 386


# A reference to a plain attribute of a class that has stores in methods only.
class Solo:
    def __init__(self):
        self.t = 0

    def other(self):
        self.t = 1


def solo(x: Solo):
    return x.t
    #        ^ defined: 398, 401


# Annotated declarations in two classes. The leaf has no member of its own.
class Base23:
    t: int = 0


class Mid23(Base23):
    t: int = 1


class Leaf23(Mid23):
    pass


def bare_leaf(x: Leaf23):
    return x.t
    #        ^ defined: 415


# Four levels. The annotation sits at the top. The levels below store `t`.
class Base24:
    t: int


class A24(Base24):
    def __init__(self):
        self.t = 0


class B24(A24):
    pass


class C24(B24):
    def __init__(self):
        self.t = 1


def four_levels(x: C24):
    return x.t
    #        ^ defined: 429
