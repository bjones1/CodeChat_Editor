# # Today's programming puzzle
#
# ## Description of the problem
#
# Detect aligned block comments. For example, in C/C++:
#
# ```C
# /* Here's a description
#    of what my_func does.
#    it's correctly indented. */
#
# /* Here's another
#   description. But it's
#    incorrectly indented. */
# void my_func() {
# }
# ```
#
# Your goal: write a program to determine if a comment is correctly indented. A
# parser has already recognized comments, and passes you the contents of each
# comment. For example:
comment_1_contents = """\
Here's a description
   of what my_func does.
   it's correctly indented.
"""

comment_2_contents = """\
Here's another
  description. But it's
   incorrectly indented.
"""

# ## Solution
#
# Notice that the parser removes the opening comment delimiter and the closing
# comment delimiter.
#
# Your task: write a function to determine if the provided comment is correctly
# indented:
def is_correctly_indented(comment: str) -> bool:
    # However, this is clearly wrong. Look at the test cases below to see what
    # the function should do. Then:
    #
    # 1.  **Before** you write code, think and write. What should this function
    #     do? How? Write your ideas below.
    # 2.  Write as little code as possible.
    # 3.  Run the tests: in the terminal, type `pytest` and press enter.
    # 4.  Explain to yourself, in writing below, why the tests failed. This is
    #     the scientific method: propose a theory that explain the observed
    #     behavior.
    # 5.  Explain what changes will correct this problem. Write comments below.
    # 6.  **Only after this writing process**, write code based on these
    #     comments.
    # 7.  Iterate.
    #
    # ### Replace this with your ideas.
    return True

# ## Tests
#
# Here are some tests to check if the code is correct.
def test_1():
    assert is_correctly_indented(comment_1_contents)

def test_2():
    assert not is_correctly_indented(comment_2_contents)

# An empty comment is correctly indented.
def test_3():
    assert is_correctly_indented("")

def test_4():
    assert is_correctly_indented("A one-line comment is always correctly indented.")

def test_4():
    assert is_correctly_indented("A one-line comment with a trailing newline is always correctly indented.\n")

def test_5():
    assert is_correctly_indented("""\
A multi-line comment

   with an empty line. This line is correctly indented.
""")

def test_6():
    assert not is_correctly_indented("""\
A multi-line comment

  with an empty line. This line is not correctly indented.
""")
