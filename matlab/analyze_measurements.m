function [N, avg, ok_samples, pass_percentage, passed] = analyze_measurements(file_path, expected_angle) 
    [~, name, ext] = fileparts(file_path);
    file_basename = sprintf("%s%s", name, ext)
    samples = readmatrix(file_path);
    N = length(samples);
    avg = mean(samples);
    std_dev = std(samples);

    ok_samples = 0;
    for x = samples'
        if abs(x - expected_angle) < 10
            ok_samples = ok_samples + 1;
        end
        
    end

    pass_percentage = (ok_samples/N) * 100;
    passed = mat2str(pass_percentage > 80);

    fprintf("Number of samples:\t%d\n", N);
    fprintf("Average value:\t\t%g°\n", round(avg, 2))
    fprintf("Standard deviation:\t%g\n", round(std_dev, 2))
    fprintf("Percent within range:\t%g%%\n", round(pass_percentage, 2));
    fprintf("Pass:\t\t\t%s\n", passed)

    % Conveniently print out the results as row of a LaTex table
    % file_basename & expected_angle & N & Avg & stddev & ok_samples & pass_percentage & test_passed
    fprintf("%s & %g & %d & %g & %g & %d & %g & %s \\\\\n",strrep(file_basename, '_', '\textunderscore '), expected_angle, N, round(avg,2), round(std_dev, 2), ok_samples, round(pass_percentage), passed);
end