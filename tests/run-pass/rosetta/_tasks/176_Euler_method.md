# Euler method

**Problem:** Euler's method numerically approximates solutions of first-order ordinary differential equations (ODEs) with a given initial value. It is an explicit method for solving initial value problems (IVPs), as described in the wikipedia page. The ODE has to be provided in the following form: {dt} = f(t,y(t)) with an initial value y(t_0) = y_0 To get a numeric solution, we replace the derivative on the LHS with a finite difference approximation: then solve for y(t+h): y(t+h) y(t) + h \, {dt} which is th

**Requirements:**
- 5 s and
- initial temperature T_0 shall be 100 °C
- room temperature T_R shall be 20 °C
- cooling constant k shall be 0.07
- time interval to calculate shall be from 0 s ──► 100 s

**Success Criteria:**
- Program produces correct output for test cases
- Implementation matches Rosetta Code specification
