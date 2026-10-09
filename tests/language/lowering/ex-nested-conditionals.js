const grade = (s) => (s >= 90 ? 'A' : s >= 80 ? 'B' : s >= 70 ? 'C' : 'F')
trace('grades', grade(95), grade(85), grade(75), grade(10))
