def split(text):
    *words, last = text.split(" ")
    print(words, last)
    #     ^ defined: 2
    #            ^ defined: 2

first, *middle, final = [1, 2, 3]
print(middle)
#     ^ defined: 7

for head, *tail in [[1, 2, 3]]:
    print(tail)
    #     ^ defined: 11
