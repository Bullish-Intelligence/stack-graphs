# More guards of `isinstance`: an assert with `and`, a condition in parentheses, a conditional
# expression, and `not isinstance(...) or ...`.

class Base:
    def base_only(self):
        pass

class Sub(Base):
    def sub_only(self):
        pass

# The statements after `assert isinstance(...) and isinstance(...)` are narrowed.
def assert_and(x):
    assert isinstance(x, Base) and isinstance(x, Sub)
    x.sub_only
    # ^ defined: 9
    x.base_only
    # ^ defined: 5

# A condition in parentheses narrows the body.
def in_parentheses(x, flag):
    if (
        flag
        and isinstance(x, Sub)
    ):
        x.sub_only
        # ^ defined: 9
    if (isinstance(x, Sub)):
        x.sub_only
        # ^ defined: 9

# The value before `if` of a conditional expression is narrowed.
def conditional(x):
    return x.sub_only if isinstance(x, Sub) else None
           # ^ defined: 9

# The right operand of `not isinstance(...) or ...` is narrowed.
def not_or(x, flag):
    if not isinstance(x, Sub) or x.sub_only:
        #                          ^ defined: 9
        return
    if not isinstance(x, Sub) or flag or x.sub_only:
        #                                  ^ defined: 9
        return

# The statements after an early exit are narrowed.
def early_exit(x, flag):
    if not isinstance(x, Sub) or flag:
        return
    x.sub_only
    # ^ defined: 9

# A narrowing does not leak.
def no_leak(x, flag):
    if not isinstance(x, Sub) or flag:
        x.sub_only
        # ^ defined:
    x.sub_only
    # ^ defined:

# Control: the plain guard.
def control(x):
    if isinstance(x, Sub):
        x.sub_only
        # ^ defined: 9
    assert isinstance(x, Sub)
    x.sub_only
    # ^ defined: 9

# A parenthesized condition of an `elif` narrows the body.
def elif_parenthesized(x, flag):
    if flag:
        pass
    elif (isinstance(x, Sub)):
        x.sub_only
        # ^ defined: 9
    elif (flag and isinstance(x, Sub)):
        x.sub_only
        # ^ defined: 9

# The call must stand in the condition slot of a conditional expression. As the value after
# `else`, it narrows nothing.
def conditional_else(x, y):
    return x.sub_only if y else isinstance(x, Sub)
           # ^ defined:

def conditional_both(x, y):
    return x.sub_only if isinstance(x, Sub) else x.sub_only
           # ^ defined: 9

# A comment between the value and the condition does not move the condition slot.
def conditional_comment(x, y):
    return (
        x.sub_only  # a comment
        # ^ defined: 9
        if isinstance(x, Sub)
        else y
    )
