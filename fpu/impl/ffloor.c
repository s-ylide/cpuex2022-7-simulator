#include <math.h>
#include "fpulib.h"
#include "fcvtws.h"
#include "fcvtsw.h"
#include "ffloor.h"

// ffloor
float ffloor(float x) {
  union Num x_num;
  x_num.real = x;
  if (slice(x_num.nat, 31, 24)>157) {
    return x;
  }

  int32_t x_int;
  float x_float;
  x_int = fcvtws(x);
  x_float = fcvtsw(x_int);
  if (x_num.real >= x_float) {
    return x_float;
  } else {
    return x_float-1.0;
  }
}
