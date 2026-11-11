def load(path):
    try:
        data = read(path)
    except OSError:
        data = None
    else:
        print(data)
        #     ^ defined: 3
    return data
    #      ^ defined: 3, 5

def clean(path):
    value = 1
    try:
        value = read(path)
    except OSError:
        pass
    else:
        print(value)
        #      ^ defined: 15
