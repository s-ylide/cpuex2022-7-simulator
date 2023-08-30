#ifndef _FPULIB_H_
#define _FPULIB_H_

#include <stdint.h>

union Num {
  float real;
  uint32_t nat;
};

union Int {
  int32_t sign;
  uint32_t unsign;
};

// left~rightまでの範囲を切り出して返す関数
uint32_t slice(uint32_t, int32_t, int32_t);

// 符号部、指数部、仮数部を受け取り、浮動小数点数を作る
uint32_t mkfloat(uint32_t, uint32_t, uint32_t);

#endif
