# A function binds a name in many forms. Each form makes the name local to the whole function, so a
# read that the flow of the function leaves unbound does not fall through to a name of the module.
# Each case binds the name in a branch that the read can skip. The module has a name of the same
# name. The module name must not be an answer.

with_target = 1
with_tuple_a = 1
with_tuple_b = 1
except_target = 1
augmented = 1
walrus = 1
walrus_comp = 1
walrus_lambda = 1
tuple_a = 1
tuple_b = 1
star_rest = 1
for_nested_a = 1
for_nested_b = 1
deleted = 1
imported = 1
imported_as = 1
from_imported = 1
from_imported_as = 1
defined_func = 1
defined_class = 1
aliased = 1
capture_seq = 1
capture_star = 1
capture_map = 1
capture_rest = 1
capture_class = 1
capture_as = 1
capture_or = 1
capture_plain = 1

def with_form(c, flag):
    if flag:
        with c as with_target:
            pass
    return with_target
#          ^ defined: 36, 38

def with_tuple_form(c, flag):
    if flag:
        with c as (with_tuple_a, with_tuple_b):
            pass
    return with_tuple_a, with_tuple_b
#          ^ defined: 45
#                        ^ defined: 45

def except_form(c):
    try:
        c()
    except ValueError as except_target:
        pass
    return except_target
#          ^ defined: 54

def augmented_form(flag):
    if flag:
        augmented += 1
    return augmented
#          ^ defined: 61

def walrus_form(c, flag):
    if flag and (walrus := c()):
        pass
    return walrus
#          ^ defined: 66

def walrus_in_comprehension(c, flag):
    if flag:
        kept = [x for x in c if (walrus_comp := x)]
    return walrus_comp
#          ^ defined: 73

# A walrus in a lambda binds in the lambda, no local of the function.
def walrus_in_lambda(c):
    f = lambda: (walrus_lambda := c)
    return walrus_lambda
#          ^ defined: 13

def tuple_form(c, flag):
    if flag:
        tuple_a, tuple_b = c
    return tuple_a, tuple_b
#          ^ defined: 85
#                   ^ defined: 85

def star_form(c, flag):
    if flag:
        first, *star_rest = c
    return star_rest
#          ^ defined: 92

def for_nested_form(c, flag):
    if flag:
        for (for_nested_a, (for_nested_b, _)) in c:
            pass
    return for_nested_a, for_nested_b
#          ^ defined: 98
#                        ^ defined: 98

# `del` makes a name local. The name in the statement is the definition.
def delete_form(flag):
    if flag:
        del deleted
    return deleted
#          ^ defined: 107

def delete_parenthesized_form(flag):
    if flag:
        del (deleted)
    return deleted
#          ^ defined: 113

def delete_list_form(flag):
    if flag:
        del [deleted, other]
    return deleted
#          ^ defined: 119

def import_form(flag):
    if flag:
        import imported
    return imported
#          ^ defined: 125

def import_as_form(flag):
    if flag:
        import something as imported_as
    return imported_as
#          ^ defined: 131

def from_import_form(flag):
    if flag:
        from somewhere import from_imported
    return from_imported
#          ^ defined: 137

def from_import_as_form(flag):
    if flag:
        from somewhere import something as from_imported_as
    return from_imported_as
#          ^ defined: 143

def def_form(flag):
    if flag:
        def defined_func():
            pass
    return defined_func
#          ^ defined: 149

def class_form(flag):
    if flag:
        class defined_class:
            pass
    return defined_class
#          ^ defined: 156

def type_alias_form(flag):
    if flag:
        type aliased = int
    return aliased
#          ^ defined: 163

def capture_form(c):
    match c:
        case [capture_seq, *capture_star]:
            pass
        case {"k": capture_map, **capture_rest}:
            pass
        case Point(x=capture_class):
            pass
        case 1 as capture_as:
            pass
    return capture_seq, capture_star, capture_map, capture_rest, capture_class, capture_as
#          ^ defined: 169
#                       ^ defined: 169
#                                     ^ defined: 171
#                                                  ^ defined: 171
#                                                                ^ defined: 173
#                                                                               ^ defined: 175

def capture_or_form(c):
    match c:
        case [1, capture_or] | [capture_or, 1]:
            pass
    return capture_or
#          ^ defined: 187

def capture_plain_form(c):
    match c:
        case 1:
            pass
        case capture_plain:
            pass
    return capture_plain
#          ^ defined: 196

# A name that `global` declares is no local of the function, in any form. It stays a name of the
# module.

g_with = 1
g_for = 1
g_except = 1
g_aug = 1
g_walrus = 1
g_tuple = 1
g_import = 1
g_def = 1
g_class = 1
g_delete = 1

def global_with(c, flag):
    global g_with
    if flag:
        with c as g_with:
            pass
    return g_with
#          ^ defined: 204

def global_for(c, flag):
    global g_for
    if flag:
        for g_for in c:
            pass
    return g_for
#          ^ defined: 205

def global_except(c):
    global g_except
    try:
        c()
    except ValueError as g_except:
        pass
    return g_except
#          ^ defined: 206

def global_augmented(flag):
    global g_aug
    if flag:
        g_aug += 1
    return g_aug
#          ^ defined: 207

def global_walrus(c, flag):
    global g_walrus
    if flag and (g_walrus := c()):
        pass
    return g_walrus
#          ^ defined: 208

def global_tuple(c, flag):
    global g_tuple
    if flag:
        g_tuple, other = c
    return g_tuple
#          ^ defined: 209

def global_import(flag):
    global g_import
    if flag:
        import g_import
    return g_import
#          ^ defined: 210

def global_def(flag):
    global g_def
    if flag:
        def g_def():
            pass
    return g_def
#          ^ defined: 211

def global_class(flag):
    global g_class
    if flag:
        class g_class:
            pass
    return g_class
#          ^ defined: 212

def global_delete(flag):
    global g_delete
    if flag:
        del g_delete
    return g_delete
#          ^ defined: 213

# A name that `nonlocal` declares names the binding of the enclosing function, in any form.

def nonlocal_forms(c, flag):
    n_with = 1
    n_for = 1
    n_aug = 1
    n_walrus = 1
    n_tuple = 1
    n_import = 1

    def inner():
        nonlocal n_with, n_for, n_aug, n_walrus, n_tuple, n_import
        if flag:
            with c as n_with:
                pass
            for n_for in c:
                pass
            n_aug += 1
            if (n_walrus := c()):
                pass
            n_tuple, other = c
            import n_import
        return n_with
#              ^ defined: 294
        return n_for
#              ^ defined: 295
        return n_aug
#              ^ defined: 296
        return n_walrus
#              ^ defined: 297
        return n_tuple
#              ^ defined: 298
        return n_import
#              ^ defined: 299

    return inner
