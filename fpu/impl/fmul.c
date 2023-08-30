#include <math.h>
#include "fpulib.h"
#include "fmul.h"

// fmul
float fmul(float x1, float x2) {
  union Num x1_num, x2_num;
  x1_num.real = x1;
  x2_num.real = x2;

  // 符号部
  uint32_t s1, s2;
  s1 = x1_num.nat >> 31;
  s2 = x2_num.nat >> 31;

  // 指数部
  uint32_t e1, e2, es;
  e1 = slice(x1_num.nat, 31, 24);
  e2 = slice(x2_num.nat, 31, 24);
  es = slice(e1+e2+129, 9, 1);

  // 仮数部上位と下位
  uint32_t h1, h2, l1, l2;
  h1 = slice(x1_num.nat, 23, 12) | 0x00001000;
  h2 = slice(x2_num.nat, 23, 12) | 0x00001000;
  l1 = slice(x1_num.nat, 11, 1);
  l2 = slice(x2_num.nat, 11, 1);

  uint32_t hh, hl, lh, mm;
  hh = h1 * h2;
  hl = h1 * l2;
  lh = l1 * h2;
  mm = hh + (hl>>11) + (lh>>11) + 2;

  // 出力の符号部、指数部、仮数部
  uint32_t sy, ey, my;
  sy = s1 ^ s2;
  if ((es>>8)==0) {
    ey = 0;
  } else if (mm>>25) {
    ey = slice(es+1, 8, 1);
  } else {
    ey = slice(es, 8, 1);
  }
  if (e1==0 || e2==0 || ey==0) {
    my = 0;
  } else if (mm>>25) {
    my = slice(mm, 25, 3);
  } else {
    my = slice(mm, 24, 2);
  }

  union Num y_num;
  y_num.nat = mkfloat(sy, ey, my);

  return y_num.real;
}
