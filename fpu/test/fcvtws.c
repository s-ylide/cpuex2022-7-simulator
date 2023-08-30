#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include "../impl/fpulib.h"
#include "../impl/fcvtws.h"


// 実装基準を満たすかのテスト用
void test_fcvtws(void) {
  uint32_t m;
  union Num x_num;
  union Int y_true, y;
  for (int32_t i = 1; i < 158; ++i) {
    for (int32_t s = 0; s < 2; ++s) {
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
        y_true.sign = (int32_t) x_num.real;
        y.sign = fcvtws(x_num.real);
        if (fabs((float)y_true.sign-x_num.real) < fabs((float)y.sign-x_num.real)) {
          printf("%f %u %d %d %u %u\n", x_num.real, x_num.nat, y_true.sign, y.sign, y_true.unsign, y.unsign);
          puts("here");
        }
      }
    }
  }
}

// Verilogの方と出力が一致するかのテスト用
void test_fcvtws_emu(void) {
  FILE *fp;
  fp = fopen("fcvtws_emu.txt", "w");
  uint32_t m;
  union Num x_num;
  union Int y;
  for (int32_t i = 1; i < 158; ++i) {
    for (int32_t s = 0; s < 2; ++s) {
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
        y.sign = fcvtws(x_num.real);
        for (int32_t k = 0; k < 32; k++) {
          int32_t bin = 1 & ((x_num.nat) >> (31 - k));
          fprintf(fp, "%d", bin);
        }
        fprintf(fp, "\n");
        for (int32_t k = 0; k < 32; k++) {
          int32_t bin = 1 & ((y.unsign) >> (31 - k));
          fprintf(fp, "%d", bin);
        }
        fprintf(fp, "\n");
      }
    }
  }
  fclose(fp);
}
