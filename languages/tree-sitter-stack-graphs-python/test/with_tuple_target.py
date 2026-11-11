def open_pair(factory):
    with factory() as (reader, writer):
        reader.read()
        # ^ defined: 2
        writer.write()
        # ^ defined: 2

    with factory() as [one, [two, *rest]]:
        print(one, two, rest)
        #     ^ defined: 8
        #          ^ defined: 8
        #               ^ defined: 8
