#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include "../impl/fpulib.h"
#include "../impl/fmul.h"
#include "../impl/fsqrt.h"

// 実装基準を満たすかのテスト用
void test_fsqrt(void) {
  uint32_t m;
  union Num x_num, y_true, y_num;
  for (int32_t i = 1; i < 254; ++i) {
    for (int32_t s = 0; s < 1; ++s) {
      for (int32_t it = 0; it < 10; ++it) {
        switch (it) {
          case 0 : m = 0; break;
          case 1 : m = 1; break;
          case 2 : m = 2; break;
          case 3 : m = 0x380000; break;
          case 4 : m = 0x400000; break;
          case 5 : m = 0x5fffff; break;
          case 6 : m = 0x7fffff; break;
          default : m = slice(rand(), 23, 1); break;
        }
        x_num.nat = mkfloat(s, i, m);
        y_true.real = sqrtf(x_num.real);
        y_num.real = fsqrt(x_num.real);
        if (fabs(y_num.real-y_true.real) >= fabs(y_true.real)*pow(2,-20)
            && fabs(y_num.real-y_true.real) >= pow(2,-126)
            && slice(y_true.nat, 31, 24) != 0
            && slice(y_true.nat, 31, 24) != 255) {
          printf("%f %f %f\n", x_num.real, y_true.real, y_num.real);
          printf("%u %u %u\n", x_num.nat, y_true.nat, y_num.nat);
          puts("here");
        }
      }
    }
  }
}

// Verilogの方と出力が一致するかのテスト用
void test_fsqrt_emu(void) {
  FILE *fp;
  fp = fopen("fsqrt_emu.txt", "w");
  uint32_t m;
  union Num x_num, y_num;
  for (int32_t i = 1; i < 254; ++i) {
    for (int32_t s = 0; s < 1; ++s) {
      for (int32_t it = 0; it < 10; ++it) {
        switch (it) {
          case 0 : m = 0; break;
          case 1 : m = 1; break;
          case 2 : m = 2; break;
          case 3 : m = 0x380000; break;
          case 4 : m = 0x400000; break;
          case 5 : m = 0x5fffff; break;
          case 6 : m = 0x7fffff; break;
          default : m = slice(rand(), 23, 1); break;
        }
        x_num.nat = mkfloat(s, i, m);
        y_num.real = fsqrt(x_num.real);
        for (int32_t k = 0; k < 32; k++) {
          int32_t bin = 1 & ((x_num.nat) >> (31 - k));
          fprintf(fp, "%d", bin);
        }
        fprintf(fp, "\n");
        for (int32_t k = 0; k < 32; k++) {
          int32_t bin = 1 & ((y_num.nat) >> (31 - k));
          fprintf(fp, "%d", bin);
        }
        fprintf(fp, "\n");
      }
    }
  }
  fclose(fp);
}
