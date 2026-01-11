local ud = createud()

assert(type(ud) == 'userdata')
assert(ud:method1() == 'abc')
