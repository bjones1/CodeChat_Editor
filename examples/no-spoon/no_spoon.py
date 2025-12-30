# `no_spoon.py` - a CodinGame challenge
# =====================================
#
# This file demonstrates the use of literate programming in solving the
# [There is no Spoon](https://www.codingame.com/ide/puzzle/there-is-no-spoon-episode-1)
# programming challenge.
#
# Imports
# -------

import sys
import math

# Input
# -----
#
# Don't let the machines win. You are humanity's last hope...
#
# First, read the input provided by the challenge:
#
# The number of cells on the X axis.
width = int(input())
# The number of cells on the Y axis.
height = int(input())
# Read in a grid of cell:
#
# * Each line is `width` characters.
# * The characters are either `0` (occupied) or `.` (empty).
#
# From the website, here's an example grid:
#
# ![](example_grid.png)
#
# The text which creates this grid is:
#
# ```
# 00
# 0.
# ```
#
# Store this an an array of lines; each line contains these characters.
# Therefore, `line[y][x]` gives the cell at the coordinate $(x,y)$ (note the
# reversed order).
line = []
for y in range(height):
    line.append(input())

# Processing and output
# ---------------------
#
# From the rules:
#
# > To do this, you must find each (x1,y1) coordinates containing a node, and
# > display the (x2,y2) coordinates of the next node to the right, and the
# > (x3,y3) coordinates of the next node to the bottom within the grid.
# >
# > If a neighbor does not exist, you must output the coordinates -1 -1 instead
# > of (x2,y2) and/or (x3,y3).
#
# ### Approach
#
# Terminology:
#
# * A cell is one point in the grid. It may be empty or occupied.
# * A node is an occupied cell.
#
# Variable naming: based on the rules, define:
#
# * (`x1`, `y1`) is the coordinate of an (occupied) node.
# * (`x2`, `y2`) is the coordinate of the next node to the right.
# * (`x3`, `y3`) is the coordinates of the next node to the bottom.
#
# ### Implementation
#
# * Loop through each cell. If it's occupied (a node):
#
#   * Look for a node to its right; if not found, return coordinates of
#     $(−1,−1)$.
#   * Look for a node to the bottom; if not found, return coordinates of
#     $(−1,−1)$.
for x1 in range(width):
    for y1 in range(height):
        if line[y1][x1] == "0":
            # This cell is occupied. First, search for the next node to the
            # right (note the `+ 1`) with the same y coordinate but a greater x
            # coordinate:
            y2 = y1
            for x2 in range(x1 + 1, width):
                if line[y2][x2] == "0":
                    break
            else:
                # This runs only if we didn't find an occupied node.
                x2 = -1
                y2 = -1

            # Do the same thing, but along the y axis.
            x3 = x1
            for y3 in range(y1 + 1, height):
                if line[y3][x3] == "0":
                    break
            else:
                x3 = -1
                y3 = -1

            print(f"{x1} {y1} {x2} {y2} {x3} {y3}")