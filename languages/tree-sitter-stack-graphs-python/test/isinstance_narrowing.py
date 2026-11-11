# Inside a block that `isinstance(x, C)` guards, the receiver `x` behaves like an instance of `C`.

class Base:
    def shared(self):
        pass

    def base_only(self):
        pass

class Sub(Base):
    def shared(self):
        pass

    def sub_only(self):
        pass

class Other:
    def other_only(self):
        pass

    def shared(self):
        pass

class Holder:
    def __init__(self):
        self.item: Base = Base()

# The body of an `if` narrows the receiver. The narrowed class shadows the declared type.
# An inherited member resolves through the narrowed class.
def if_body(x: Base, y):
    if isinstance(x, Sub):
        x.sub_only
        # ^ defined: 14
        x.shared
        # ^ defined: 11
        x.base_only
        # ^ defined: 7
        x
      # ^ defined: 30
        y.sub_only
        # ^ defined:
    x.sub_only
    # ^ defined:
    x.shared
    # ^ defined: 4

# Each branch of an `elif` chain gets its own class only.
def elif_chain(x):
    if isinstance(x, Sub):
        x.shared
        # ^ defined: 11
    elif isinstance(x, Other):
        x.shared
        # ^ defined: 21
    else:
        x.shared
        # ^ defined:

# When the narrowed class lacks the member, the declared type resolves it.
def fallback(x: Base):
    if isinstance(x, Other):
        x.base_only
        # ^ defined: 7

# Nested statements in the body are narrowed too. An inner guard shadows an outer guard.
def nested(x, items):
    if isinstance(x, Base):
        for item in items:
            x.base_only
            # ^ defined: 7
            if isinstance(x, Sub):
                x.shared
                # ^ defined: 11
            x.shared
            # ^ defined: 4

# An `assert` narrows the rest of its block only.
def asserted(x, flag):
    if flag:
        x.shared
        # ^ defined:
        assert isinstance(x, Other)
        x.other_only
        # ^ defined: 18
    x.other_only
    # ^ defined:

# An early exit narrows the rest of its block.
def early_exit(x, xs):
    if not isinstance(x, Sub):
        return
    x.sub_only
    # ^ defined: 14
    for y in xs:
        if not isinstance(y, Other):
            continue
        y.other_only
        # ^ defined: 18

# An `if` with an `else` narrows nothing after it.
def no_early_exit(x):
    if not isinstance(x, Sub):
        return
    else:
        pass
    x.sub_only
    # ^ defined:

# A tuple of classes gives one candidate for each class that has the member.
def tuple_classes(x):
    if isinstance(x, (Sub, Other)):
        x.sub_only
        # ^ defined: 14
        x.other_only
        # ^ defined: 18
        x.shared
        # ^ defined: 11, 21

# An attribute of a name narrows the same attribute as a receiver.
def attribute_subject(h: Holder):
    if isinstance(h.item, Sub):
        h.item.sub_only
        #      ^ defined: 14
        h.item
        # ^ defined: 26
    h.item.sub_only
    #      ^ defined:

# A guard in an `and` chain narrows the body and the operands after it.
def and_chain(x, y, flag):
    if isinstance(x, Sub) and x.sub_only:
        #                       ^ defined: 14
        x.sub_only
        # ^ defined: 14
    if flag and isinstance(x, Other) and y:
        x.other_only
        # ^ defined: 18
    if x.sub_only and isinstance(x, Sub):
        #^ defined:
        pass

# A comment in the argument list does not break the file.
def with_comment(x):
    if isinstance(  # a comment
        x, Sub
    ):
        pass
    y = isinstance(x, Sub)  # a comment
    z = isinstance(
        x,  # a comment
        Sub,
    )
    x.shared
    # ^ defined:
