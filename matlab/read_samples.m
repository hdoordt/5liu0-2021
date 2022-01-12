function samples = read_samples(path)
    s = readmatrix(path);
    ch1 = s(:,1).';
    ch2 = s(:,2).';
    ch3 = s(:,3).';
    ch4 = s(:,4).';
    len = length(s);
    n = linspace(1, len, len);

    samples.ch1 = ch1;
    samples.ch2 = ch2;
    samples.ch3 = ch3;
    samples.ch4 = ch4;
    samples.ch1_mean = ch1 - mean(ch1);
    samples.ch2_mean = ch2 - mean(ch2);
    samples.ch3_mean = ch3 - mean(ch3);
    samples.ch4_mean = ch4 - mean(ch4);
    samples.s = s;
    samples.len = len;
    samples.n = n;
end