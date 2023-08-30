#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <limits.h>
#include "../impl/fpulib.h"
#include "../impl/fcvtsw.h"

// 実装基準を満たすかのテスト用
void test_fcvtsw(void) {
  union Int x_int;
  union Num y_true, y;
  for (unsigned long long i = 0; i <= UINT_MAX; ++i) {
    x_int.unsign = (uint32_t) i;
    y_true.real = (float) x_int.sign;
    y.real = fcvtsw(x_int.sign);
    if (abs((int32_t)y_true.real-x_int.sign) < abs((int32_t)y.real-x_int.sign)) {
      printf("%d %u %f %f %u %u\n", x_int.sign, x_int.unsign, y_true.real, y.real, y_true.nat, y.nat);
      puts("here");
    }
  }
}

// Verilogの方と出力が一致するかのテスト用
void test_fcvtsw_emu(void) {
  FILE *fp;
  fp = fopen("fcvtsw_emu.txt", "w");
  union Int x_int;
  union Num y;
  for (unsigned long long i = 0; i <= UINT_MAX; i += 1024*1023+1) {
    x_int.unsign = (uint32_t) i;
    y.real = fcvtsw(x_int.sign);
    for (int32_t k = 0; k < 32; k++) {
      int32_t bin = 1 & ((x_int.unsign) >> (31 - k));
      fprintf(fp, "%d", bin);
    }
    fprintf(fp, "\n");
    for (int32_t k = 0; k < 32; k++) {
      int32_t bin = 1 & ((y.nat) >> (31 - k));
      fprintf(fp, "%d", bin);
    }
    fprintf(fp, "\n");
  }
  fclose(fp);
}
