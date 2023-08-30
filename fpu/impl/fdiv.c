#include <math.h>
#include "fpulib.h"
#include "fmul.h"
#include "fdiv.h"

// fdiv
float fdiv(float x1, float x2) {
  union Num x1_num, x2_num;
  x1_num.real = x1;
  x2_num.real = x2;

  // 符号部
  uint32_t s1, s2;
  s1 = x1_num.nat >> 31;
  s2 = x2_num.nat >> 31;

  // 指数部
  uint32_t e1, e2;
  e1 = slice(x1_num.nat, 31, 24);
  e2 = slice(x2_num.nat, 31, 24);

  // 仮数部
  uint32_t m1, m2, h;
  union Num m1n, m2n;
  m1 = slice(x1_num.nat, 23, 1);
  m2 = slice(x2_num.nat, 23, 1);
  h = slice(m2, 23, 14);
  m1n.nat = mkfloat(0, 127, m1);
  m2n.nat = mkfloat(0, 127, m2);

  // 線形近似の傾きと切片
  // 数値誤差を抑えるために面倒な計算をしている
  double d_grad, d_intercept;
  d_grad = 1024.0 * (1024.0/(1024.0+(double)h) - 1024.0/(1025.0+(double)h));
  d_intercept = 1024.0*(1.0 - (1024.0+(double)h)/(1025.0+(double)h)) + (768.0/(1024.0+(double)h) - 256/(1025.0+(double)h) + 1024/(2049+(double)(2*h)));
  float grad, intercept;
  grad = (float) d_grad;
  intercept = (float) d_intercept;

  // 逆数計算
  float ax, m2inv;
  ax = fmul(grad, m2n.real);
  m2inv = intercept - ax;

  union Num mdiv;
  mdiv.real = fmul(m1n.real, m2inv);
  uint32_t ovf, udf;
  ovf = slice(mdiv.nat, 31, 31);
  udf = slice(~mdiv.nat, 24, 24);

  // 出力の符号部、指数部、仮数部
  uint32_t sy, ey, my;
  sy = s1 ^ s2;
  ey = slice(e1-e2+127-udf+ovf, 8, 1);
  my = slice(mdiv.nat, 23, 1);

  union Num y_num;
  y_num.nat = mkfloat(sy, ey, my);

  return y_num.real;
}
