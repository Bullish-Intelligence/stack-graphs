value = "module"
total = 0

def use_it(items):
    if (found := len(items)) > 1:
        print(found)
#             ^ defined: 5
    return found
#          ^ defined: 5

def in_comprehension(items):
    kept = [y for x in items if (y := x + 1) > 0]
    print(y)
#         ^ defined: 12
    return kept

def shadows_module():
    # The walrus makes `total` a name of the function, not of the module.
    while (total := total_of()) > 0:
        print(total)
#             ^ defined: 19

def total_of():
    return 1

if (n := total_of()) > 0:
    print(n)
#         ^ defined: 26

data = [z for z in range(3) if (last := z) > 1]
print(last)
#     ^ defined: 30

class Holder:
    if (flag := True):
        pass
    print(flag)
#         ^ defined: 35

# The value of the expression keeps its members.
class Item:
    size = 1

def members():
    if (item := Item()):
        print(item.size)
#                  ^ defined: 42
