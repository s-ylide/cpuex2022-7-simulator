#include <math.h>
#include "fpulib.h"
#include "fcvtsw.h"

// fcvtsw
float fcvtsw(int32_t x) {
  union Int x_int;
  x_int.sign = x;

  // 符号部
  uint32_t s, xabs, sa, xs;
  s = x_int.unsign >> 31;
  if (x >= 0) {
    xabs = (uint32_t) x;
  } else {
    xabs = (uint32_t) -x;
  }
  for (int32_t i = 31; i >= 0; --i) {
    if (xabs & (1<<i)) {
      sa = 32 - i;
      break;
    }
    if (i == 0) {
      sa = 0;
    }
  }
  if (sa == 32) {
    xs = 0;
  } else {
    xs = xabs << sa;
  }

  // 計算結果
  uint32_t ey, my;
  if (sa == 0) {
    ey = 0;
  } else if ((xs>>9) == 0x7fffff && slice(xs,9,9)) {
    ey = 127 - sa + 33;
  } else {
    ey = 127 - sa + 32;
  }
  my = slice((xs>>9)+slice(xs, 9, 9), 23, 1);

  union Num y;
  y.nat = mkfloat(s, ey, my);

  return y.real;
}

