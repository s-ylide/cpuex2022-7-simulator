#include <math.h>
#include "fpulib.h"
#include "fcvtws.h"

// fcvtws
int32_t fcvtws(float x) {
  union Num x_num;
  x_num.real = x;

  // 符号部
  uint32_t s;
  s = x_num.nat >> 31;

  // 指数部
  uint32_t e, sa, sai;
  e = slice(x_num.nat, 31, 24);
  sa = 157 - e;
  sai = sa - 1;

  // 仮数部
  uint32_t m, me, mes, mesi, mesr;
  m = slice(x_num.nat, 23, 1);
  me = (1<<30) + (m<<7);
  if (sa > 31) {
    mes = 0;
  } else {
    mes = me >> sa;
  }
  if (sai > 31) {
    mesi = 0;
  } else {
    mesi = me >> sai;
  }
  if (mesi & 1) {
    mesr = mes + 1;
  } else {
    mesr = mes;
  }

  // 計算結果
  int32_t mesrc, y;
  mesrc = (~mesr | 0x80000000) + 1;
  if (s == 0) {
    y = (int32_t) mesr;
  } else {
    y = mesrc;
  }

  return y;
}
