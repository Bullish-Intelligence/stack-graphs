counter = 0
cache = None

def bump():
    global counter
    #      ^ defined: 1
    counter = counter + 1
    # ^ defined: 1
    #         ^ defined: 1
    return counter
    #      ^ defined: 1

def fill():
    global cache
    if cache is None:
        cache = {}
    return cache
    #      ^ defined: 2

def outer():
    seen = False

    def mark():
        nonlocal seen
        #        ^ defined: 21
        seen = True
        # ^ defined: 21

    mark()
    return seen
    #      ^ defined: 21

def local_only():
    counter = 5
    return counter
    #      ^ defined: 34
