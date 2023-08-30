#include <math.h>
#include <stdint.h>
#include "fpulib.h"
#include "fmul.h"
#include "fsqrt.h"

// fsqrt
float fsqrt(float x) {
  union Num x_num;
  x_num.real = x;

  // 符号部
  uint32_t s;
  s = x_num.nat >> 31;

  // 指数部
  uint32_t e;
  e = slice(x_num.nat, 31, 24);

  // 仮数部
  uint32_t m, h;
  union Num mn;
  m = slice(x_num.nat, 23, 1);
  h = slice(x_num.nat, 24, 15) ^ 0x00000200;
  if (e & 1) {
    mn.nat = mkfloat(0, 127, m);
  } else {
    mn.nat = mkfloat(0, 128, m);
  }

  // 線形近似の傾きと切片
  // 数値誤差を抑えるために面倒な計算をしている
  double d_grad, d_intercept;
  if (h < 512) {
    d_grad = 512.0 * (sqrt((double)(513+h)/512.0) - sqrt((double)(512+h)/512.0));
    d_intercept = (2.0*sqrt((double)(1025+2*h)/1024.0) + sqrt((double)(513+h)/512.0) + sqrt((double)(512+h)/512.0)) / 4.0 - ((double)(1025+2*h)/2.0) * (sqrt((double)(513+h)/512.0) - sqrt((double)(512+h)/512.0));
  } else {
    d_grad = 256.0 * (sqrt((double)(1+h)/256.0) - sqrt((double)h/256.0));
    d_intercept = (2.0*sqrt((double)(1+2*h)/512.0) + sqrt((double)(1+h)/256.0) + sqrt((double)h/256.0)) / 4.0 - ((double)(1+2*h)/2.0) * (sqrt((double)(1+h)/256.0) - sqrt((double)h/256.0));
  }
  float grad, intercept;
  grad = (float) d_grad;
  intercept = (float) d_intercept;

  // 平方根計算
  float ax;
  union Num msqrt;
  ax = fmul(grad, mn.real);
  msqrt.real = intercept + ax;

  // 出力の符号部、指数部、仮数部
  uint32_t ey, my;
  if (e==255 || e==0) {
    ey = 0;
  } else {
    ey = (e-127)/2 + 127;
  }
  my = slice(msqrt.nat, 23, 1);

  union Num y_num;
  y_num.nat = mkfloat(s, ey, my);

  return y_num.real;
}
