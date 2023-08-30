#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <limits.h>
#include "../impl/fpulib.h"
#include "../impl/fcvtws.h"
#include "../impl/fcvtsw.h"
#include "../impl/ffloor.h"

// 実装基準を満たすかのテスト用
void test_ffloor(void) {
  union Num x_num, y;
  for (unsigned long long i = 0; i <= UINT_MAX; ++i) {
    x_num.nat = (uint32_t) i;
    //y_true.real = floorf(x_num.real);
    y.real = ffloor(x_num.real);
    if ((y.real > x_num.real) || (y.real+1.0 <= x_num.real)) {
      printf("%f %u %f %u\n", x_num.real, x_num.nat, y.real, y.nat);
      puts("here");
    }
  }
}

// Verilogの方と出力が一致するかのテスト用
void test_ffloor_emu(void) {
  FILE *fp;
  fp = fopen("ffloor_emu.txt", "w");
  union Num x_num, y;
  for (unsigned long long i = 0; i <= UINT_MAX; i += 1024 * 1023 + 1) {
    x_num.nat = (uint32_t) i;
    y.real = ffloor(x_num.real);
    for (int32_t k = 0; k < 32; k++) {
      int32_t bin = 1 & ((x_num.nat) >> (31 - k));
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
