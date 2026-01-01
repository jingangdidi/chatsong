#!/usr/bin/python
# -*- coding: utf-8 -*-
import os
import sys

a = None
b = None

for i in range(1, len(sys.argv)):
	if sys.argv[i] == '--a':
		a = int(sys.argv[i + 1])
	elif sys.argv[i] == '--b':
		b = int(sys.argv[i + 1])

if a is None:
	print('error: missing --a')
elif b is None:
	print('error: missing --b')
else:
	result = a + b
	print('{} + {} = {}'.format(a, b, result))
