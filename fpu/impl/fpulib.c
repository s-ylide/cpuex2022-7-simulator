#include "fpulib.h"

// left~rightまでの範囲を切り出して返す関数
uint32_t slice(uint32_t x, int32_t left, int32_t right) {
  return (x<<(32-left)) >> (31-left+right);
}

// 符号部、指数部、仮数部を受け取り、浮動小数点数を作る
uint32_t mkfloat(uint32_t s, uint32_t e, uint32_t m) {
  return (s<<31) + (e<<23) + m;
}
